// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::api::EventReadApiServer;
use crate::api::EventStreamingApiServer;
use crate::api::TransactionStreamingApiServer;
use crate::SuiRpcModule;
use async_trait::async_trait;
use futures::{StreamExt, TryStream};
use jsonrpsee::core::RpcResult;
use jsonrpsee::types::SubscriptionResult;
use jsonrpsee_core::error::SubscriptionClosed;
use jsonrpsee_core::server::rpc_module::RpcModule;
use jsonrpsee_core::server::rpc_module::SubscriptionSink;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::ModuleId;
use serde::Serialize;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;
use sui_core::authority::AuthorityState;
use sui_core::event_handler::EventHandler;
use sui_core::transaction_streamer::TransactionStreamer;
use sui_json_rpc_types::SuiCertifiedTransaction;
use sui_json_rpc_types::SuiTransactionEffects;
use sui_json_rpc_types::SuiTransactionFilter;
use sui_json_rpc_types::{SuiEvent, SuiEventEnvelope, SuiEventFilter};
use sui_open_rpc::Module;
use sui_types::base_types::{ObjectID, SuiAddress, TransactionDigest};
use sui_types::object::Owner;
use tracing::warn;

pub struct TransactionStreamingApiImpl {
    state: Arc<AuthorityState>,
    transaction_streamer: Arc<TransactionStreamer>,
}

impl TransactionStreamingApiImpl {
    pub fn new(state: Arc<AuthorityState>, transaction_streamer: Arc<TransactionStreamer>) -> Self {
        Self {
            state,
            transaction_streamer,
        }
    }
}

#[async_trait]
impl TransactionStreamingApiServer for TransactionStreamingApiImpl {
    fn subscribe_transaction(
        &self,
        mut sink: SubscriptionSink,
        filter: SuiTransactionFilter,
    ) -> SubscriptionResult {
        let filter = match filter.try_into() {
            Ok(filter) => filter,
            Err(e) => {
                let e = jsonrpsee_core::Error::from(e);
                warn!(error = ?e, "Rejecting subscription request.");
                return Ok(sink.reject(e)?);
            }
        };

        let state = self.state.clone();
        let stream = self.transaction_streamer.subscribe(filter);
        let stream = stream.map(move |(tx_cert, signed_effects)| {
            SuiCertifiedTransaction::try_from(tx_cert).and_then(|tx_cert| {
                SuiTransactionEffects::try_from(signed_effects.effects, state.module_cache.as_ref())
                    .and_then(|effects| Ok((tx_cert, effects)))
            })
            // let sui_tx_cert  = SuiCertifiedTransaction::try_from(tx_cert);
            // sui_tx_cert.map(|tx_cert| {
            //     let sui_signed_effects = SuiTransactionEffects::try_from(signed_effects.effects, state.module_cache.as_ref())?;
            //     (tx_cert, sui_signed_effects)
            // })
        });
        spawn_subscription(sink, stream);

        Ok(())
    }
}

impl SuiRpcModule for TransactionStreamingApiImpl {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        crate::api::TransactionStreamingApiOpenRpc::module_doc()
    }
}

pub fn spawn_subscription<S, T, E>(mut sink: SubscriptionSink, rx: S)
where
    S: TryStream<Ok = T, Error = E> + Unpin + Send + 'static,
    T: Serialize,
    E: Display,
{
    tokio::spawn(async move {
        match sink.pipe_from_try_stream(rx).await {
            SubscriptionClosed::Success => {
                sink.close(SubscriptionClosed::Success);
            }
            SubscriptionClosed::RemotePeerAborted => (),
            SubscriptionClosed::Failed(err) => {
                warn!(error = ?err, "Event subscription closed.");
                sink.close(err);
            }
        };
    });
}
