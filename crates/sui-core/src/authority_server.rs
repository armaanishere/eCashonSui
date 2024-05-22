// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use async_trait::async_trait;
use mysten_metrics::histogram::Histogram as MystenHistogram;
use mysten_metrics::spawn_monitored_task;
use narwhal_worker::LazyNarwhalClient;
use prometheus::{
    register_int_counter_vec_with_registry, register_int_counter_with_registry, IntCounter,
    IntCounterVec, Registry,
};
use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::SystemTime,
};
use sui_network::{
    api::{Validator, ValidatorServer},
    tonic,
};
use sui_types::effects::TransactionEffectsAPI;
use sui_types::messages_consensus::ConsensusTransaction;
use sui_types::messages_grpc::{HandleCertificateRequestV3, HandleCertificateResponseV3};
use sui_types::messages_grpc::{
    HandleCertificateResponseV2, HandleTransactionResponse, ObjectInfoRequest, ObjectInfoResponse,
    SubmitCertificateResponse, SystemStateRequest, TransactionInfoRequest, TransactionInfoResponse,
};
use sui_types::messages_grpc::{
    HandleSoftBundleCertificatesRequestV3, HandleSoftBundleCertificatesResponseV3,
};
use sui_types::multiaddr::Multiaddr;
use sui_types::sui_system_state::SuiSystemState;
use sui_types::traffic_control::{ClientIdSource, PolicyConfig, RemoteFirewallConfig, Weight};
use sui_types::{error::*, transaction::*};
use sui_types::{
    fp_ensure,
    messages_checkpoint::{
        CheckpointRequest, CheckpointRequestV2, CheckpointResponse, CheckpointResponseV2,
    },
};
use tap::TapFallible;
use tokio::task::JoinHandle;
use tracing::{error, error_span, info, Instrument};

use crate::authority::authority_per_epoch_store::AuthorityPerEpochStore;
use crate::{
    authority::AuthorityState,
    consensus_adapter::{ConsensusAdapter, ConsensusAdapterMetrics},
    traffic_controller::policies::TrafficTally,
    traffic_controller::TrafficController,
};
use crate::{
    consensus_adapter::ConnectionMonitorStatusForTests,
    traffic_controller::metrics::TrafficControllerMetrics,
};
use tonic::transport::server::TcpConnectInfo;

#[cfg(test)]
#[path = "unit_tests/server_tests.rs"]
mod server_tests;

pub struct AuthorityServerHandle {
    tx_cancellation: tokio::sync::oneshot::Sender<()>,
    local_addr: Multiaddr,
    handle: JoinHandle<Result<(), tonic::transport::Error>>,
}

impl AuthorityServerHandle {
    pub async fn join(self) -> Result<(), io::Error> {
        // Note that dropping `self.complete` would terminate the server.
        self.handle
            .await?
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }

    pub async fn kill(self) -> Result<(), io::Error> {
        self.tx_cancellation.send(()).map_err(|_e| {
            io::Error::new(io::ErrorKind::Other, "could not send cancellation signal!")
        })?;
        self.handle
            .await?
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }

    pub fn address(&self) -> &Multiaddr {
        &self.local_addr
    }
}

pub struct AuthorityServer {
    address: Multiaddr,
    pub state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    pub metrics: Arc<ValidatorServiceMetrics>,
}

impl AuthorityServer {
    pub fn new_for_test(
        address: Multiaddr,
        state: Arc<AuthorityState>,
        consensus_address: Multiaddr,
    ) -> Self {
        let consensus_adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(LazyNarwhalClient::new(consensus_address)),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
            state.epoch_store_for_testing().protocol_config().clone(),
        ));

        let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());

        Self {
            address,
            state,
            consensus_adapter,
            metrics,
        }
    }

    pub async fn spawn_for_test(self) -> Result<AuthorityServerHandle, io::Error> {
        let address = self.address.clone();
        self.spawn_with_bind_address_for_test(address).await
    }

    pub async fn spawn_with_bind_address_for_test(
        self,
        address: Multiaddr,
    ) -> Result<AuthorityServerHandle, io::Error> {
        let mut server = mysten_network::config::Config::new()
            .server_builder()
            .add_service(ValidatorServer::new(ValidatorService {
                state: self.state,
                consensus_adapter: self.consensus_adapter,
                metrics: self.metrics.clone(),
                traffic_controller: None,
                client_id_source: None,
            }))
            .bind(&address)
            .await
            .unwrap();
        let local_addr = server.local_addr().to_owned();
        info!("Listening to traffic on {local_addr}");
        let handle = AuthorityServerHandle {
            tx_cancellation: server.take_cancel_handle().unwrap(),
            local_addr,
            handle: spawn_monitored_task!(server.serve()),
        };
        Ok(handle)
    }
}

pub struct ValidatorServiceMetrics {
    pub signature_errors: IntCounter,
    pub tx_verification_latency: MystenHistogram,
    pub cert_verification_latency: MystenHistogram,
    pub consensus_latency: MystenHistogram,
    pub handle_transaction_latency: MystenHistogram,
    pub submit_certificate_consensus_latency: MystenHistogram,
    pub handle_certificate_consensus_latency: MystenHistogram,
    pub handle_certificate_non_consensus_latency: MystenHistogram,

    num_rejected_tx_in_epoch_boundary: IntCounter,
    num_rejected_cert_in_epoch_boundary: IntCounter,
    num_rejected_tx_during_overload: IntCounterVec,
    num_rejected_cert_during_overload: IntCounterVec,
    connection_ip_not_found: IntCounter,
    forwarded_header_parse_error: IntCounter,
    forwarded_header_invalid: IntCounter,
    forwarded_header_not_included: IntCounter,
}

impl ValidatorServiceMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            signature_errors: register_int_counter_with_registry!(
                "total_signature_errors",
                "Number of transaction signature errors",
                registry,
            )
            .unwrap(),
            tx_verification_latency: MystenHistogram::new_in_registry(
                "validator_service_tx_verification_latency",
                "Latency of verifying a transaction",
                registry,
            ),
            cert_verification_latency: MystenHistogram::new_in_registry(
                "validator_service_cert_verification_latency",
                "Latency of verifying a certificate",
                registry,
            ),
            consensus_latency: MystenHistogram::new_in_registry(
                "validator_service_consensus_latency",
                "Time spent between submitting a shared obj txn to consensus and getting result",
                registry,
            ),
            handle_transaction_latency: MystenHistogram::new_in_registry(
                "validator_service_handle_transaction_latency",
                "Latency of handling a transaction",
                registry,
            ),
            handle_certificate_consensus_latency: MystenHistogram::new_in_registry(
                "validator_service_handle_certificate_consensus_latency",
                "Latency of handling a consensus transaction certificate",
                registry,
            ),
            submit_certificate_consensus_latency: MystenHistogram::new_in_registry(
                "validator_service_submit_certificate_consensus_latency",
                "Latency of submit_certificate RPC handler",
                registry,
            ),
            handle_certificate_non_consensus_latency: MystenHistogram::new_in_registry(
                "validator_service_handle_certificate_non_consensus_latency",
                "Latency of handling a non-consensus transaction certificate",
                registry,
            ),
            num_rejected_tx_in_epoch_boundary: register_int_counter_with_registry!(
                "validator_service_num_rejected_tx_in_epoch_boundary",
                "Number of rejected transaction during epoch transitioning",
                registry,
            )
            .unwrap(),
            num_rejected_cert_in_epoch_boundary: register_int_counter_with_registry!(
                "validator_service_num_rejected_cert_in_epoch_boundary",
                "Number of rejected transaction certificate during epoch transitioning",
                registry,
            )
            .unwrap(),
            num_rejected_tx_during_overload: register_int_counter_vec_with_registry!(
                "validator_service_num_rejected_tx_during_overload",
                "Number of rejected transaction due to system overload",
                &["error_type"],
                registry,
            )
            .unwrap(),
            num_rejected_cert_during_overload: register_int_counter_vec_with_registry!(
                "validator_service_num_rejected_cert_during_overload",
                "Number of rejected transaction certificate due to system overload",
                &["error_type"],
                registry,
            )
            .unwrap(),
            connection_ip_not_found: register_int_counter_with_registry!(
                "validator_service_connection_ip_not_found",
                "Number of times connection IP was not extractable from request",
                registry,
            )
            .unwrap(),
            forwarded_header_parse_error: register_int_counter_with_registry!(
                "validator_service_forwarded_header_parse_error",
                "Number of times x-forwarded-for header could not be parsed",
                registry,
            )
            .unwrap(),
            forwarded_header_invalid: register_int_counter_with_registry!(
                "validator_service_forwarded_header_invalid",
                "Number of times x-forwarded-for header was invalid",
                registry,
            )
            .unwrap(),
            forwarded_header_not_included: register_int_counter_with_registry!(
                "validator_service_forwarded_header_not_included",
                "Number of times x-forwarded-for header was (unexpectedly) not included in request",
                registry,
            )
            .unwrap(),
        }
    }

    pub fn new_for_tests() -> Self {
        let registry = Registry::new();
        Self::new(&registry)
    }
}

#[derive(Clone)]
pub struct ValidatorService {
    state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    metrics: Arc<ValidatorServiceMetrics>,
    traffic_controller: Option<Arc<TrafficController>>,
    client_id_source: Option<ClientIdSource>,
}

impl ValidatorService {
    pub fn new(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        validator_metrics: Arc<ValidatorServiceMetrics>,
        traffic_controller_metrics: TrafficControllerMetrics,
        policy_config: Option<PolicyConfig>,
        firewall_config: Option<RemoteFirewallConfig>,
    ) -> Self {
        Self {
            state,
            consensus_adapter,
            metrics: validator_metrics,
            traffic_controller: policy_config.clone().map(|policy| {
                Arc::new(TrafficController::spawn(
                    policy,
                    traffic_controller_metrics,
                    firewall_config,
                ))
            }),
            client_id_source: policy_config.map(|policy| policy.client_id_source),
        }
    }

    pub fn validator_state(&self) -> &Arc<AuthorityState> {
        &self.state
    }

    pub async fn execute_certificate_for_testing(
        &self,
        cert: CertifiedTransaction,
    ) -> Result<tonic::Response<HandleCertificateResponseV2>, tonic::Status> {
        let request = make_tonic_request_for_testing(cert);
        self.handle_certificate_v2(request).await
    }

    pub async fn handle_transaction_for_benchmarking(
        &self,
        transaction: Transaction,
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        let request = make_tonic_request_for_testing(transaction);
        self.transaction(request).await
    }

    async fn handle_transaction(
        &self,
        request: tonic::Request<Transaction>,
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        let Self {
            state,
            consensus_adapter,
            metrics,
            traffic_controller: _,
            client_id_source: _,
        } = self.clone();
        let transaction = request.into_inner();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        // CRITICAL: DO NOT ADD ANYTHING BEFORE THIS CHECK.
        // This must be the first thing to check before anything else, because the transaction
        // may not even be valid to access for any other checks.
        transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch())?;

        // When authority is overloaded and decide to reject this tx, we still lock the object
        // and ask the client to retry in the future. This is because without locking, the
        // input objects can be locked by a different tx in the future, however, the input objects
        // may already be locked by this tx in other validators. This can cause non of the txes
        // to have enough quorum to form a certificate, causing the objects to be locked for
        // the entire epoch. By doing locking but pushback, retrying transaction will have
        // higher chance to succeed.
        let mut validator_pushback_error = None;
        let overload_check_res = state.check_system_overload(
            &consensus_adapter,
            transaction.data(),
            state.check_system_overload_at_signing(),
        );
        if let Err(error) = overload_check_res {
            metrics
                .num_rejected_tx_during_overload
                .with_label_values(&[error.as_ref()])
                .inc();
            // TODO: consider change the behavior for other types of overload errors.
            match error {
                SuiError::ValidatorOverloadedRetryAfter { .. } => {
                    validator_pushback_error = Some(error)
                }
                _ => return Err(error.into()),
            }
        }

        let _handle_tx_metrics_guard = metrics.handle_transaction_latency.start_timer();

        let tx_verif_metrics_guard = metrics.tx_verification_latency.start_timer();
        let transaction = epoch_store.verify_transaction(transaction).tap_err(|_| {
            metrics.signature_errors.inc();
        })?;
        drop(tx_verif_metrics_guard);

        let tx_digest = transaction.digest();

        // Enable Trace Propagation across spans/processes using tx_digest
        let span = error_span!("validator_state_process_tx", ?tx_digest);

        let info = state
            .handle_transaction(&epoch_store, transaction)
            .instrument(span)
            .await
            .tap_err(|e| {
                if let SuiError::ValidatorHaltedAtEpochEnd = e {
                    metrics.num_rejected_tx_in_epoch_boundary.inc();
                }
            })?;

        if let Some(error) = validator_pushback_error {
            // TODO: right now, we still sign the txn, but just don't return it. We can also skip signing
            // to save more CPU.
            return Err(error.into());
        }
        Ok(tonic::Response::new(info))
    }

    async fn handle_certificates(
        &self,
        certificates: Vec<CertifiedTransaction>,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
        _include_auxiliary_data: bool,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        wait_for_effects: bool,
    ) -> Result<Option<Vec<HandleCertificateResponseV3>>, tonic::Status> {
        // Validate if cert can be executed
        // Fullnode does not serve handle_certificate call.
        fp_ensure!(
            !self.state.is_fullnode(epoch_store),
            SuiError::FullNodeCantHandleCertificate.into()
        );

        fp_ensure!(
            !certificates.is_empty(),
            SuiError::NoCertificateProvided.into()
        );

        // See [SIP-19](https://github.com/sui-foundation/sips/blob/main/sips/sip-19.md) for more details.
        let is_soft_bundle = certificates.len() > 1;
        if is_soft_bundle {
            let protocol_config = epoch_store.protocol_config();
            // The request is a Soft Bundle, enforce these checks per SIP-19:
            // - All certs must access at least one shared object.
            // - All certs must not be already executed.
            // - All certs must have the same gas price.
            // - Number of certs must not exceed the max allowed.
            fp_ensure!(
                certificates.len() as u64 <= protocol_config.max_soft_bundle_size(),
                SuiError::UserInputError {
                    error: UserInputError::TooManyTransactionsInSoftBundle {
                        limit: protocol_config.max_soft_bundle_size()
                    }
                }
                .into()
            );
            let mut gas_price = None;
            for certificate in &certificates {
                let tx_digest = *certificate.digest();
                fp_ensure!(
                    certificate.contains_shared_object(),
                    SuiError::UserInputError {
                        error: UserInputError::NoSharedObjectError { digest: tx_digest }
                    }
                    .into()
                );
                fp_ensure!(
                    !self.state.is_tx_already_executed(&tx_digest)?,
                    SuiError::UserInputError {
                        error: UserInputError::AlreadyExecutedError { digest: tx_digest }
                    }
                    .into()
                );
                if let Some(gas) = gas_price {
                    fp_ensure!(
                        gas == certificate.gas_price(),
                        SuiError::UserInputError {
                            error: UserInputError::GasPriceMismatchError {
                                digest: tx_digest,
                                expected: gas,
                                actual: certificate.gas_price()
                            }
                        }
                        .into()
                    );
                } else {
                    gas_price = Some(certificate.gas_price());
                }
            }
        }

        let shared_object_tx = is_soft_bundle || certificates[0].contains_shared_object();

        let _metrics_guard = if wait_for_effects {
            if shared_object_tx {
                self.metrics
                    .handle_certificate_consensus_latency
                    .start_timer()
            } else {
                self.metrics
                    .handle_certificate_non_consensus_latency
                    .start_timer()
            }
        } else {
            self.metrics
                .submit_certificate_consensus_latency
                .start_timer()
        };

        let mut responses = Vec::with_capacity(certificates.len());

        // 1) Check if the certificate is already executed.
        //    This is only needed when we have only one certificate (not a soft bundle).
        if !is_soft_bundle {
            assert!(certificates.len() == 1); // Guranateed by the above check.
            let tx_digest = *certificates[0].digest();

            if let Some(signed_effects) = self
                .state
                .get_signed_effects_and_maybe_resign(&tx_digest, epoch_store)?
            {
                let events = if include_events {
                    if let Some(digest) = signed_effects.events_digest() {
                        Some(self.state.get_transaction_events(digest)?)
                    } else {
                        None
                    }
                } else {
                    None
                };

                return Ok(Some(vec![HandleCertificateResponseV3 {
                    effects: signed_effects.into_inner(),
                    events,
                    input_objects: None,
                    output_objects: None,
                    auxiliary_data: None,
                }]));
            };
        }

        // 2) Verify the certificates.
        // Check system overload
        for certificate in &certificates {
            let overload_check_res = self.state.check_system_overload(
                &self.consensus_adapter,
                certificate.data(),
                self.state.check_system_overload_at_execution(),
            );
            if let Err(error) = overload_check_res {
                self.metrics
                    .num_rejected_cert_during_overload
                    .with_label_values(&[error.as_ref()])
                    .inc();
                return Err(error.into());
            }
        }

        let mut verified_certificates = Vec::with_capacity(certificates.len());
        for certificate in certificates {
            let _timer = self.metrics.cert_verification_latency.start_timer();
            let verified_certificate = epoch_store
                .signature_verifier
                .verify_cert(certificate)
                .await?;
            verified_certificates.push(verified_certificate);
        }

        let submit_to_consensus = || -> SuiResult<()> {
            // code block within reconfiguration lock
            let reconfiguration_lock = epoch_store.get_reconfig_state_read_lock_guard();
            if !reconfiguration_lock.should_accept_user_certs() {
                self.metrics.num_rejected_cert_in_epoch_boundary.inc();
                return Err(SuiError::ValidatorHaltedAtEpochEnd.into());
            }

            // 3) All certificates are sent to consensus (at least by some authorities)
            // For shared objects this will wait until either timeout or we have heard back from consensus.
            // For owned objects this will return without waiting for certificate to be sequenced
            // First do quick dirty non-async check
            if !epoch_store.is_all_tx_certs_consensus_message_processed(&verified_certificates)? {
                let _metrics_guard = if shared_object_tx {
                    Some(self.metrics.consensus_latency.start_timer())
                } else {
                    None
                };
                let transactions = verified_certificates
                    .iter()
                    .map(|certificate| {
                        ConsensusTransaction::new_certificate_message(
                            &self.state.name,
                            certificate.clone().into(),
                        )
                    })
                    .collect::<Vec<_>>();
                self.consensus_adapter.submit_batch(
                    &transactions,
                    Some(&reconfiguration_lock),
                    epoch_store,
                )?;
                // Do not wait for the result, because the transaction might have already executed.
                // Instead, check or wait for the existence of certificate effects below.
            }
            drop(reconfiguration_lock);
            Ok(())
        };

        submit_to_consensus()?;

        if !wait_for_effects {
            // It is useful to enqueue owned object transaction for execution locally,
            // even when we are not returning effects to user
            let certificates_without_shared_objects = verified_certificates
                .iter()
                .filter(|certificate| !certificate.contains_shared_object())
                .cloned()
                .collect::<Vec<_>>();
            if !certificates_without_shared_objects.is_empty() {
                self.state.enqueue_certificates_for_execution(
                    certificates_without_shared_objects,
                    epoch_store,
                );
            }
            return Ok(None);
        }

        for certificate in verified_certificates {
            // 4) Execute the certificate if it contains only owned object transactions, or wait for
            // the execution results if it contains shared objects.
            // TODO: for soft bundle, we can execute all the certs at once.
            let response = {
                let effects = self
                    .state
                    .execute_certificate(&certificate, epoch_store)
                    .await?;
                let events = if include_events {
                    if let Some(digest) = effects.events_digest() {
                        Some(self.state.get_transaction_events(digest)?)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let input_objects = include_input_objects
                    .then(|| self.state.get_transaction_input_objects(&effects))
                    .and_then(Result::ok);

                let output_objects = include_output_objects
                    .then(|| self.state.get_transaction_output_objects(&effects))
                    .and_then(Result::ok);

                HandleCertificateResponseV3 {
                    effects: effects.into_inner(),
                    events,
                    input_objects,
                    output_objects,
                    auxiliary_data: None, // We don't have any aux data generated presently
                }
            };
            responses.push(response);
        }

        Ok(Some(responses))
    }
}

impl ValidatorService {
    async fn transaction_impl(
        &self,
        request: tonic::Request<Transaction>,
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        self.handle_transaction(request).await
    }

    async fn submit_certificate_impl(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<SubmitCertificateResponse>, tonic::Status> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let certificate = request.into_inner();
        Self::transaction_validity_check(&epoch_store, certificate.data())?;

        let span = error_span!("submit_certificate", tx_digest = ?certificate.digest());
        self.handle_certificates(
            vec![certificate],
            true,
            false,
            false,
            false,
            &epoch_store,
            false,
        )
        .instrument(span)
        .await
        .map(|executed| {
            tonic::Response::new(SubmitCertificateResponse {
                executed: executed.map(|mut x| x.remove(0)).map(Into::into),
            })
        })
    }

    async fn handle_certificate_v2_impl(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<HandleCertificateResponseV2>, tonic::Status> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let certificate = request.into_inner();
        Self::transaction_validity_check(&epoch_store, certificate.data())?;

        let span = error_span!("handle_certificate", tx_digest = ?certificate.digest());
        self.handle_certificates(
            vec![certificate],
            true,
            false,
            false,
            false,
            &epoch_store,
            true,
        )
        .instrument(span)
        .await
        .map(|v| {
            tonic::Response::new(
                v.expect("handle_certificate should not return none with wait_for_effects=true")
                    .remove(0)
                    .into(),
            )
        })
    }

    async fn handle_certificate_v3_impl(
        &self,
        request: tonic::Request<HandleCertificateRequestV3>,
    ) -> Result<tonic::Response<HandleCertificateResponseV3>, tonic::Status> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let request = request.into_inner();
        Self::transaction_validity_check(&epoch_store, request.certificate.data())?;

        let span = error_span!("handle_certificate_v3", tx_digest = ?request.certificate.digest());
        self.handle_certificates(
            vec![request.certificate],
            request.include_events,
            request.include_input_objects,
            request.include_output_objects,
            request.include_auxiliary_data,
            &epoch_store,
            true,
        )
        .instrument(span)
        .await
        .map(|v| {
            tonic::Response::new(
                v.expect("handle_certificate should not return none with wait_for_effects=true")
                    .remove(0),
            )
        })
    }

    async fn handle_soft_bundle_certificates_v3_impl(
        &self,
        request: tonic::Request<HandleSoftBundleCertificatesRequestV3>,
    ) -> Result<tonic::Response<HandleSoftBundleCertificatesResponseV3>, tonic::Status> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let protocol_config = epoch_store.protocol_config();
        let node_config = &self.state.config;
        // Soft Bundle MUST be enabled both in protocol config and local node config.
        // This acts an extra safety measure where a validator node have the choice to turn this feature off, without
        // having to upgrade the entire network.
        fp_ensure!(
            protocol_config.soft_bundle() && node_config.enable_soft_bundle,
            SuiError::UnsupportedFeatureError {
                error: "Soft Bundle".to_string()
            }
            .into()
        );

        let request = request.into_inner();
        let certificates = request.certificates;

        for certificate in &certificates {
            // CRITICAL: DO NOT ADD ANYTHING BEFORE THIS CHECK.
            // This must be the first thing to check before anything else, because the transaction
            // may not even be valid to access for any other checks.
            // We need to check this first because we haven't verified the cert signature.
            Self::transaction_validity_check(&epoch_store, certificate.data())?;
        }

        let span = error_span!("handle_soft_bundle_certificates_v3");
        self.handle_certificates(
            certificates,
            request.include_events,
            request.include_input_objects,
            request.include_output_objects,
            request.include_auxiliary_data,
            &epoch_store,
            request.wait_for_effects,
        )
        .instrument(span)
        .await
        .map(|v| {
            tonic::Response::new(HandleSoftBundleCertificatesResponseV3 {
                responses: v.unwrap_or_default(),
            })
        })
    }

    fn transaction_validity_check(
        epoch_store: &Arc<AuthorityPerEpochStore>,
        transaction: &SenderSignedData,
    ) -> SuiResult<()> {
        let config = epoch_store.protocol_config();
        transaction.validity_check(config, epoch_store.epoch())?;
        // TODO: The following check should be moved into
        // TransactionData::check_version_and_features_supported.
        // However that's blocked by some tests that uses randomness features
        // even when the protocol feature is not enabled.
        if !epoch_store.randomness_state_enabled()
            && transaction.transaction_data().uses_randomness()
        {
            return Err(SuiError::UnsupportedFeatureError {
                error: "randomness is not enabled on this network".to_string(),
            });
        }
        Ok(())
    }

    async fn object_info_impl(
        &self,
        request: tonic::Request<ObjectInfoRequest>,
    ) -> Result<tonic::Response<ObjectInfoResponse>, tonic::Status> {
        let request = request.into_inner();
        let response = self.state.handle_object_info_request(request).await?;
        Ok(tonic::Response::new(response))
    }

    async fn transaction_info_impl(
        &self,
        request: tonic::Request<TransactionInfoRequest>,
    ) -> Result<tonic::Response<TransactionInfoResponse>, tonic::Status> {
        let request = request.into_inner();
        let response = self.state.handle_transaction_info_request(request).await?;
        Ok(tonic::Response::new(response))
    }

    async fn checkpoint_impl(
        &self,
        request: tonic::Request<CheckpointRequest>,
    ) -> Result<tonic::Response<CheckpointResponse>, tonic::Status> {
        let request = request.into_inner();
        let response = self.state.handle_checkpoint_request(&request)?;
        Ok(tonic::Response::new(response))
    }

    async fn checkpoint_v2_impl(
        &self,
        request: tonic::Request<CheckpointRequestV2>,
    ) -> Result<tonic::Response<CheckpointResponseV2>, tonic::Status> {
        let request = request.into_inner();
        let response = self.state.handle_checkpoint_request_v2(&request)?;
        Ok(tonic::Response::new(response))
    }

    async fn get_system_state_object_impl(
        &self,
        _request: tonic::Request<SystemStateRequest>,
    ) -> Result<tonic::Response<SuiSystemState>, tonic::Status> {
        let response = self
            .state
            .get_object_cache_reader()
            .get_sui_system_state_object_unsafe()?;

        Ok(tonic::Response::new(response))
    }

    async fn handle_traffic_req(&self, client: Option<IpAddr>) -> Result<(), tonic::Status> {
        if let Some(traffic_controller) = &self.traffic_controller {
            if !traffic_controller.check(&client, &None).await {
                // Entity in blocklist
                Err(tonic::Status::from_error(SuiError::TooManyRequests.into()))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn handle_traffic_resp<T>(
        &self,
        client: Option<IpAddr>,
        response: &Result<tonic::Response<T>, tonic::Status>,
    ) {
        let error: Option<SuiError> = if let Err(status) = response {
            Some(SuiError::from(status.clone()))
        } else {
            None
        };

        if let Some(traffic_controller) = self.traffic_controller.clone() {
            traffic_controller.tally(TrafficTally {
                direct: client,
                through_fullnode: None,
                error_weight: error.map(normalize).unwrap_or(Weight::zero()),
                timestamp: SystemTime::now(),
            })
        }
    }
}

fn make_tonic_request_for_testing<T>(message: T) -> tonic::Request<T> {
    // simulate a TCP connection, which would have added extensions to
    // the request object that would be used downstream
    let mut request = tonic::Request::new(message);
    let tcp_connect_info = TcpConnectInfo {
        local_addr: None,
        remote_addr: Some(SocketAddr::new([127, 0, 0, 1].into(), 0)),
    };
    request.extensions_mut().insert(tcp_connect_info);
    request
}

// TODO: refine error matching here
fn normalize(err: SuiError) -> Weight {
    match err {
        SuiError::UserInputError { .. }
        | SuiError::InvalidSignature { .. }
        | SuiError::SignerSignatureAbsent { .. }
        | SuiError::SignerSignatureNumberMismatch { .. }
        | SuiError::IncorrectSigner { .. }
        | SuiError::UnknownSigner { .. }
        | SuiError::WrongEpoch { .. } => Weight::one(),
        _ => Weight::zero(),
    }
}

/// Implements generic pre- and post-processing. Since this is on the critical
/// path, any heavy lifting should be done in a separate non-blocking task
/// unless it is necessary to override the return value.
#[macro_export]
macro_rules! handle_with_decoration {
    ($self:ident, $func_name:ident, $request:ident) => {{
        if $self.client_id_source.is_none() {
            return $self.$func_name($request).await;
        }

        let client = match $self.client_id_source.as_ref().unwrap() {
            ClientIdSource::SocketAddr => {
                let socket_addr: Option<SocketAddr> = $request.remote_addr();

                // We will hit this case if the IO type used does not
                // implement Connected or when using a unix domain socket.
                // TODO: once we have confirmed that no legitimate traffic
                // is hitting this case, we should reject such requests that
                // hit this case.
                if let Some(socket_addr) = socket_addr {
                    Some(socket_addr.ip())
                } else {
                    if cfg!(msim) {
                        // Ignore the error from simtests.
                    } else if cfg!(test) {
                        panic!("Failed to get remote address from request");
                    } else {
                        $self.metrics.connection_ip_not_found.inc();
                        error!("Failed to get remote address from request");
                    }
                    None
                }
            }
            ClientIdSource::XForwardedFor => {
                if let Some(op) = $request.metadata().get("x-forwarded-for") {
                    match op.to_str() {
                        Ok(header_val) => {
                            match header_val.parse::<SocketAddr>() {
                                Ok(socket_addr) => Some(socket_addr.ip()),
                                Err(err) => {
                                    $self.metrics.forwarded_header_parse_error.inc();
                                    error!(
                                        "Failed to parse x-forwarded-for header value of {:?} to ip address: {:?}. \
                                        Please ensure that your proxy is configured to resolve client domains to an \
                                        IP address before writing header",
                                        header_val,
                                        err,
                                    );
                                    None
                                }
                            }
                        }
                        Err(e) => {
                            // TODO: once we have confirmed that no legitimate traffic
                            // is hitting this case, we should reject such requests that
                            // hit this case.
                            $self.metrics.forwarded_header_invalid.inc();
                            error!("Invalid UTF-8 in x-forwarded-for header: {:?}", e);
                            None
                        }
                    }
                } else {
                    $self.metrics.forwarded_header_not_included.inc();
                    error!("x-forwarded-header not present for request despite node configuring XForwardedFor tracking type");
                    None
                }
            }
        };

        // check if either IP is blocked, in which case return early
        $self.handle_traffic_req(client.clone()).await?;
        // handle request
        let response = $self.$func_name($request).await;
        // handle response tallying
        $self.handle_traffic_resp(client, &response);
        response
    }};
}

#[async_trait]
impl Validator for ValidatorService {
    async fn transaction(
        &self,
        request: tonic::Request<Transaction>,
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        let validator_service = self.clone();

        // Spawns a task which handles the transaction. The task will unconditionally continue
        // processing in the event that the client connection is dropped.
        spawn_monitored_task!(async move {
            // NB: traffic tally wrapping handled within the task rather than on task exit
            // to prevent an attacker from subverting traffic control by severing the connection
            handle_with_decoration!(validator_service, transaction_impl, request)
        })
        .await
        .unwrap()
    }

    async fn submit_certificate(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<SubmitCertificateResponse>, tonic::Status> {
        let validator_service = self.clone();

        // Spawns a task which handles the certificate. The task will unconditionally continue
        // processing in the event that the client connection is dropped.
        spawn_monitored_task!(async move {
            // NB: traffic tally wrapping handled within the task rather than on task exit
            // to prevent an attacker from subverting traffic control by severing the connection.
            handle_with_decoration!(validator_service, submit_certificate_impl, request)
        })
        .await
        .unwrap()
    }

    async fn handle_certificate_v2(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<HandleCertificateResponseV2>, tonic::Status> {
        handle_with_decoration!(self, handle_certificate_v2_impl, request)
    }

    async fn handle_certificate_v3(
        &self,
        request: tonic::Request<HandleCertificateRequestV3>,
    ) -> Result<tonic::Response<HandleCertificateResponseV3>, tonic::Status> {
        handle_with_decoration!(self, handle_certificate_v3_impl, request)
    }

    async fn handle_soft_bundle_certificates_v3(
        &self,
        request: tonic::Request<HandleSoftBundleCertificatesRequestV3>,
    ) -> Result<tonic::Response<HandleSoftBundleCertificatesResponseV3>, tonic::Status> {
        handle_with_decoration!(self, handle_soft_bundle_certificates_v3_impl, request)
    }

    async fn object_info(
        &self,
        request: tonic::Request<ObjectInfoRequest>,
    ) -> Result<tonic::Response<ObjectInfoResponse>, tonic::Status> {
        handle_with_decoration!(self, object_info_impl, request)
    }

    async fn transaction_info(
        &self,
        request: tonic::Request<TransactionInfoRequest>,
    ) -> Result<tonic::Response<TransactionInfoResponse>, tonic::Status> {
        handle_with_decoration!(self, transaction_info_impl, request)
    }

    async fn checkpoint(
        &self,
        request: tonic::Request<CheckpointRequest>,
    ) -> Result<tonic::Response<CheckpointResponse>, tonic::Status> {
        handle_with_decoration!(self, checkpoint_impl, request)
    }

    async fn checkpoint_v2(
        &self,
        request: tonic::Request<CheckpointRequestV2>,
    ) -> Result<tonic::Response<CheckpointResponseV2>, tonic::Status> {
        handle_with_decoration!(self, checkpoint_v2_impl, request)
    }

    async fn get_system_state_object(
        &self,
        request: tonic::Request<SystemStateRequest>,
    ) -> Result<tonic::Response<SuiSystemState>, tonic::Status> {
        handle_with_decoration!(self, get_system_state_object_impl, request)
    }
}
