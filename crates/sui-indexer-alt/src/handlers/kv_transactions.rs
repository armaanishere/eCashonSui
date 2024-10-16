// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::{Context, Result};
use diesel_async::RunQueryDsl;
use sui_types::full_checkpoint_content::CheckpointData;

use crate::{db, models::transactions::StoredTransaction, schema::kv_transactions};

use super::Handler;

pub struct KvTransactions;

#[async_trait::async_trait]
impl Handler for KvTransactions {
    const NAME: &'static str = "kv_transactions";

    const BATCH_SIZE: usize = 100;
    const CHUNK_SIZE: usize = 1000;
    const MAX_PENDING_SIZE: usize = 10000;

    type Value = StoredTransaction;

    fn handle(checkpoint: &Arc<CheckpointData>) -> Result<Vec<Self::Value>> {
        let CheckpointData {
            transactions,
            checkpoint_summary,
            ..
        } = checkpoint.as_ref();

        let mut values = Vec::with_capacity(transactions.len());
        let first_tx = checkpoint_summary.network_total_transactions as usize - transactions.len();

        for (i, tx) in transactions.iter().enumerate() {
            let tx_sequence_number = (first_tx + i) as i64;
            let transaction = &tx.transaction.data().intent_message().value;
            let effects = &tx.effects;
            let events: Vec<_> = tx.events.iter().flat_map(|e| e.data.iter()).collect();

            values.push(StoredTransaction {
                tx_sequence_number: (first_tx + i) as i64,
                cp_sequence_number: checkpoint_summary.sequence_number as i64,
                timestamp_ms: checkpoint_summary.timestamp_ms as i64,
                raw_transaction: bcs::to_bytes(transaction)
                    .with_context(|| format!("Serializing transaction {tx_sequence_number}"))?,
                raw_effects: bcs::to_bytes(effects).with_context(|| {
                    format!("Serializing effects for transaction {tx_sequence_number}")
                })?,
                events: bcs::to_bytes(&events).with_context(|| {
                    format!("Serializing events for transaction {tx_sequence_number}")
                })?,
            });
        }

        Ok(values)
    }

    async fn commit(values: &[Self::Value], conn: &mut db::Connection<'_>) -> Result<usize> {
        Ok(diesel::insert_into(kv_transactions::table)
            .values(values)
            .on_conflict_do_nothing()
            .execute(conn)
            .await?)
    }
}
