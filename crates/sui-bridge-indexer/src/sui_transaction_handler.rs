use crate::postgres_manager::{update_sui_progress_store, write, PgPool, PgProgressStore};
use crate::metrics::BridgeIndexerMetrics;
use crate::{BridgeDataSource, TokenTransfer, TokenTransferData, TokenTransferStatus};
use anyhow::Result;
use futures::StreamExt;
use sui_types::digests::TransactionDigest;

use std::time::Duration;
use sui_bridge::events::{
    MoveTokenDepositedEvent, MoveTokenTransferApproved, MoveTokenTransferClaimed,
};

use sui_json_rpc_types::{
    SuiTransactionBlockEffectsAPI, SuiTransactionBlockResponse,
};

use sui_types::BRIDGE_ADDRESS;
use tracing::{error, info};

pub(crate) const COMMIT_BATCH_SIZE: usize = 10;

pub async fn handle_sui_transcations_loop(
    pg_pool: PgPool,
    rx: mysten_metrics::metered_channel::Receiver<(Vec<SuiTransactionBlockResponse>, Option<TransactionDigest>)>,
    metrics: BridgeIndexerMetrics,
) {
    let checkpoint_commit_batch_size = std::env::var("COMMIT_BATCH_SIZE")
        .unwrap_or(COMMIT_BATCH_SIZE.to_string())
        .parse::<usize>()
        .unwrap();
    let mut stream = mysten_metrics::metered_channel::ReceiverStream::new(rx)
        .ready_chunks(checkpoint_commit_batch_size);
    while let Some(batch) = stream.next().await {
        // unwrap: batch must not be empty
        let cursor = batch.last().unwrap().1.clone();
        let token_transfers = batch.into_iter().map(
            // TODO: letting it panic so we can capture errors, but we should handle this more gracefully
            |(chunk, _)| process_transctions(chunk, &metrics).unwrap()
        ).flatten().collect::<Vec<_>>();
        // for (chunk, _) in batch {
        //     let token_transfers = process_transctions(resp, &metrics).unwrap();
            if !token_transfers.is_empty() {
                while let Err(err) = write(&pg_pool, token_transfers.clone()) {
                    error!("Failed to write sui transactions to DB: {:?}", err);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                info!("Wrote {} token transfers to DB", token_transfers.len());
            }
        // }
        if let Some(cursor) = cursor {
            while let Err(err) = update_sui_progress_store(&pg_pool, cursor.clone()) {
                error!("Failed to update sui progress tore DB: {:?}", err);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            info!("Updated sui transaction cursor to {}", cursor);
        }
    }
    unreachable!("Channel closed unexpectedly");
}

fn process_transctions(
    resp: Vec<SuiTransactionBlockResponse>,
    metrics: &BridgeIndexerMetrics,
) -> Result<Vec<TokenTransfer>> {
    resp.into_iter()
        .map(|r| into_token_transfers(r, metrics))
        .collect::<Result<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect())
}

pub fn into_token_transfers(
    resp: SuiTransactionBlockResponse,
    metrics: &BridgeIndexerMetrics,
) -> Result<Vec<TokenTransfer>> {
    let mut transfers = Vec::new();
    let tx_digest = resp.digest;
    let events = resp.events.ok_or(anyhow::anyhow!(
        "Expected events in SuiTransactionBlockResponse: {:?}",
        tx_digest
    ))?;
    let checkpoint_num = resp.checkpoint.ok_or(anyhow::anyhow!(
        "Expected checkpoint in SuiTransactionBlockResponse: {:?}",
        tx_digest
    ))?;
    let timestamp_ms = resp.timestamp_ms.ok_or(anyhow::anyhow!(
        "Expected timestamp_ms in SuiTransactionBlockResponse: {:?}",
        tx_digest
    ))?;
    let effects = resp.effects.ok_or(anyhow::anyhow!(
        "Expected effects in SuiTransactionBlockResponse: {:?}",
        tx_digest
    ))?;
    for ev in events.data {
        if ev.type_.address != BRIDGE_ADDRESS {
            continue;
        }
        match ev.type_.name.as_str() {
            "TokenDepositedEvent" => {
                info!("Observed Sui Deposit {:?}", ev);
                metrics.total_sui_token_deposited.inc();
                let move_event: MoveTokenDepositedEvent = bcs::from_bytes(&ev.bcs)?;
                transfers.push(TokenTransfer {
                    chain_id: move_event.source_chain,
                    nonce: move_event.seq_num,
                    block_height: checkpoint_num,
                    timestamp_ms,
                    txn_hash: tx_digest.inner().to_vec(),
                    txn_sender: ev.sender.to_vec(),
                    status: TokenTransferStatus::Deposited,
                    gas_usage: effects.gas_cost_summary().net_gas_usage(),
                    data_source: BridgeDataSource::Sui,
                    data: Some(TokenTransferData {
                        destination_chain: move_event.target_chain,
                        sender_address: move_event.sender_address.clone(),
                        recipient_address: move_event.target_address.clone(),
                        token_id: move_event.token_type,
                        amount: move_event.amount_sui_adjusted,
                    }),
                });
            }
            "TokenTransferApproved" => {
                info!("Observed Sui Approval {:?}", ev);
                metrics.total_sui_token_transfer_approved.inc();
                let event: MoveTokenTransferApproved = bcs::from_bytes(&ev.bcs)?;
                transfers.push(TokenTransfer {
                    chain_id: event.message_key.source_chain,
                    nonce: event.message_key.bridge_seq_num,
                    block_height: checkpoint_num,
                    timestamp_ms,
                    txn_hash: tx_digest.inner().to_vec(),
                    txn_sender: ev.sender.to_vec(),
                    status: TokenTransferStatus::Approved,
                    gas_usage: effects.gas_cost_summary().net_gas_usage(),
                    data_source: BridgeDataSource::Sui,
                    data: None,
                });
            }
            "TokenTransferClaimed" => {
                info!("Observed Sui Claim {:?}", ev);
                metrics.total_sui_token_transfer_claimed.inc();
                let event: MoveTokenTransferClaimed = bcs::from_bytes(&ev.bcs)?;
                transfers.push(TokenTransfer {
                    chain_id: event.message_key.source_chain,
                    nonce: event.message_key.bridge_seq_num,
                    block_height: checkpoint_num,
                    timestamp_ms,
                    txn_hash: tx_digest.inner().to_vec(),
                    txn_sender: ev.sender.to_vec(),
                    status: TokenTransferStatus::Claimed,
                    gas_usage: effects.gas_cost_summary().net_gas_usage(),
                    data_source: BridgeDataSource::Sui,
                    data: None,
                });
            }
            _ => {
                metrics.total_sui_bridge_txn_other.inc();
            }
        }
    }
    if !transfers.is_empty() {
        info!(
            ?tx_digest,
            "SUI: Extracted {} bridge token transfer data entries",
            transfers.len(),
        );
    }
    Ok(transfers)
}
