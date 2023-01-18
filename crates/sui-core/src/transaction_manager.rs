// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use parking_lot::RwLock;
use sui_types::{base_types::ObjectID, committee::EpochId, storage::ObjectKey};
use sui_types::{base_types::TransactionDigest, error::SuiResult, messages::VerifiedCertificate};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, warn};

use crate::authority::authority_per_epoch_store::AuthorityPerEpochStore;
use crate::authority::{AuthorityMetrics, AuthorityStore};

/// TransactionManager is responsible for managing pending certificates and publishes a stream
/// of certificates ready to be executed. It works together with AuthorityState for receiving
/// pending certificates, and getting notified about committed objects. Executing driver
/// subscribes to the stream of ready certificates published by the TransactionManager, and can
/// execute them in parallel.
/// TODO: use TransactionManager for fullnode.
pub struct TransactionManager {
    authority_store: Arc<AuthorityStore>,
    tx_ready_certificates: UnboundedSender<VerifiedCertificate>,
    metrics: Arc<AuthorityMetrics>,
    inner: RwLock<Inner>,
}

struct PendingCertificate {
    certificate: VerifiedCertificate,
    missing: BTreeSet<ObjectKey>,
}

#[derive(Default)]
struct Inner {
    // Current epoch of TransactionManager.
    epoch: EpochId,

    // Maps missing input objects to transactions in pending_certificates.
    // Note that except for immutable objects, a given key may only have one TransactionDigest in
    // the set. Unfortunately we cannot easily verify that this invariant is upheld, because you
    // cannot determine from TransactionData whether an input is mutable or immutable.
    missing_inputs: HashMap<ObjectKey, BTreeSet<TransactionDigest>>,

    // Number of transactions that depend on each object ID. Should match exactly with total
    // number of transactions per object ID prefix in the missing_inputs table.
    // Used for throttling signing and submitting transactions depending on hot objects.
    input_objects: HashMap<ObjectID, usize>,

    // A transaction enqueued to TransactionManager must be in either pending_certificates or
    // executing_certificates.

    // Maps transactions to their missing input objects.
    pending_certificates: HashMap<TransactionDigest, PendingCertificate>,
    // Transactions that have all input objects available, but have not finished execution.
    executing_certificates: HashSet<TransactionDigest>,
}

impl Inner {
    fn new(epoch: EpochId) -> Inner {
        Inner {
            epoch,
            ..Default::default()
        }
    }
}

impl TransactionManager {
    /// If a node restarts, transaction manager recovers in-memory data from pending certificates and
    /// other persistent data.
    pub(crate) fn new(
        authority_store: Arc<AuthorityStore>,
        epoch_store: &AuthorityPerEpochStore,
        tx_ready_certificates: UnboundedSender<VerifiedCertificate>,
        metrics: Arc<AuthorityMetrics>,
    ) -> TransactionManager {
        TransactionManager {
            authority_store,
            metrics,
            inner: RwLock::new(Inner::new(epoch_store.epoch())),
            tx_ready_certificates,
        }
    }

    /// Enqueues certificates into TransactionManager. Once all of the input objects are available
    /// locally for a certificate, the certified transaction will be sent to execution driver.
    ///
    /// REQUIRED: Shared object locks must be taken before calling this function on shared object
    /// transactions!
    ///
    /// TODO: it may be less error prone to take shared object locks inside this function, or
    /// require shared object lock versions get passed in as input. But this function should not
    /// have many callsites. Investigate the alternatives here.
    pub(crate) fn enqueue(
        &self,
        certs: Vec<VerifiedCertificate>,
        epoch_store: &AuthorityPerEpochStore,
    ) -> SuiResult<()> {
        let inner = &mut self.inner.write();
        for cert in certs {
            if cert.epoch() != inner.epoch {
                error!(
                    "Received certificate from wrong epoch! Expected={} Certificate={:?}",
                    inner.epoch, cert
                );
            }
            let digest = *cert.digest();
            // hold the tx lock until we have finished checking if objects are missing, so that we
            // don't race with a concurrent execution of this tx.
            let _tx_lock = epoch_store.acquire_tx_lock(&digest);

            // skip already pending txes
            if inner.pending_certificates.contains_key(&digest) {
                self.metrics
                    .transaction_manager_num_enqueued_certificates
                    .with_label_values(&["already_pending"])
                    .inc();
                continue;
            }
            // skip already executing txes
            if inner.executing_certificates.contains(&digest) {
                self.metrics
                    .transaction_manager_num_enqueued_certificates
                    .with_label_values(&["already_executing"])
                    .inc();
                continue;
            }
            // skip already executed txes
            if self.authority_store.effects_exists(&digest)? {
                self.metrics
                    .transaction_manager_num_enqueued_certificates
                    .with_label_values(&["already_executed"])
                    .inc();
                continue;
            }

            let missing = self
                .authority_store
                .get_missing_input_objects(
                    &digest,
                    &cert.data().intent_message.value.input_objects()?,
                    epoch_store,
                )
                .expect("Are shared object locks set prior to enqueueing certificates?");

            if missing.is_empty() {
                debug!(tx_digest = ?digest, "certificate ready");
                assert!(inner.executing_certificates.insert(digest));
                self.certificate_ready(cert);
                self.metrics
                    .transaction_manager_num_enqueued_certificates
                    .with_label_values(&["ready"])
                    .inc();
                continue;
            }

            // A missing input object in TransactionManager will definitely be notified via
            // objects_committed(), when the object actually gets committed, because:
            // 1. Assume rocksdb is strongly consistent, writing the object to the objects
            // table must happen after not finding the object in get_missing_input_objects().
            // 2. Notification via objects_committed() will happen after an object is written
            // into the objects table.
            // 3. TransactionManager is protected by a mutex. The notification via
            // objects_committed() can only arrive after the current enqueue() call finishes.
            debug!(tx_digest = ?digest, ?missing, "certificate waiting on missing objects");

            for objkey in missing.iter() {
                debug!(?objkey, ?digest, "adding missing object entry");
                assert!(
                    inner
                        .missing_inputs
                        .entry(*objkey)
                        .or_default()
                        .insert(digest),
                    "Duplicated certificate {:?} for missing object {:?}",
                    digest,
                    objkey
                );
                let input_count = inner.input_objects.entry(objkey.0).or_default();
                *input_count += 1;
            }

            assert!(
                inner
                    .pending_certificates
                    .insert(
                        digest,
                        PendingCertificate {
                            certificate: cert,
                            missing: missing.into_iter().collect()
                        }
                    )
                    .is_none(),
                "Duplicated pending certificate {:?}",
                digest
            );

            self.metrics
                .transaction_manager_num_enqueued_certificates
                .with_label_values(&["pending"])
                .inc();
        }

        self.metrics
            .transaction_manager_num_missing_objects
            .set(inner.missing_inputs.len() as i64);
        self.metrics
            .transaction_manager_num_pending_certificates
            .set(inner.pending_certificates.len() as i64);
        Ok(())
    }

    /// Notifies TransactionManager that the given objects have been committed.
    pub(crate) fn objects_committed(
        &self,
        object_keys: Vec<ObjectKey>,
        epoch_store: &AuthorityPerEpochStore,
    ) {
        let mut ready_certs = Vec::new();

        {
            let inner = &mut self.inner.write();
            if inner.epoch != epoch_store.epoch() {
                warn!("Ignoring objects committed from wrong epoch. Expected={} Actual={} Objects={:?}", inner.epoch, epoch_store.epoch(), object_keys);
                return;
            }
            for object_key in object_keys {
                if let Some(digests) = inner.missing_inputs.remove(&object_key) {
                    // Clean up object ID count table.
                    let input_count = inner.input_objects.get_mut(&object_key.0).unwrap();
                    *input_count -= digests.len();
                    if *input_count == 0 {
                        inner.input_objects.remove(&object_key.0);
                    }
                    // Clean up pending certificates table.
                    for digest in digests.iter() {
                        // Pending certificate must exist.
                        let cert = inner.pending_certificates.get_mut(digest).unwrap();
                        assert!(cert.missing.remove(&object_key));
                        // When a certificate has no missing input, it is ready to execute.
                        if cert.missing.is_empty() {
                            debug!(tx_digest = ?digest, "certificate ready");
                            ready_certs.push(inner.pending_certificates.remove(digest).unwrap());
                            assert!(inner.executing_certificates.insert(*digest));
                        } else {
                            debug!(tx_digest = ?digest, missing = ?cert.missing, "Certificate waiting on missing inputs");
                        }
                    }
                } else {
                    // No pending transaction is using this object ref as input.
                    continue;
                };
            }

            self.metrics
                .transaction_manager_num_missing_objects
                .set(inner.missing_inputs.len() as i64);
            self.metrics
                .transaction_manager_num_pending_certificates
                .set(inner.pending_certificates.len() as i64);
            self.metrics
                .transaction_manager_num_executing_certificates
                .set(inner.executing_certificates.len() as i64);
        }

        for cert in ready_certs {
            self.certificate_ready(cert.certificate);
        }
    }

    /// Notifies TransactionManager about a certificate that has been executed.
    pub(crate) fn certificate_executed(
        &self,
        digest: &TransactionDigest,
        epoch_store: &AuthorityPerEpochStore,
    ) {
        {
            let inner = &mut self.inner.write();
            if inner.epoch != epoch_store.epoch() {
                warn!("Ignoring certificate committed from wrong epoch. Expected={} Actual={} CertificateDigest={:?}", inner.epoch, epoch_store.epoch(), digest);
                return;
            }
            inner.executing_certificates.remove(digest);
            self.metrics
                .transaction_manager_num_executing_certificates
                .set(inner.executing_certificates.len() as i64);
        }
    }

    /// Sends the ready certificate for execution.
    fn certificate_ready(&self, certificate: VerifiedCertificate) {
        self.metrics.transaction_manager_num_ready.inc();
        let _ = self.tx_ready_certificates.send(certificate);
    }

    // Returns the number of transactions waiting on each object ID.
    pub(crate) fn objects_queue_len(&self, keys: Vec<ObjectID>) -> Vec<(ObjectID, usize)> {
        let inner = self.inner.read();
        keys.into_iter()
            .map(|key| {
                (
                    key,
                    inner.input_objects.get(&key).cloned().unwrap_or_default(),
                )
            })
            .collect()
    }

    // Reconfigures the TransactionManager for a new epoch. Existing transactions will be dropped
    // because they are no longer relevant and may be incorrect in the new epoch.
    pub(crate) fn reconfigure(&self, new_epoch: EpochId) {
        let mut inner = self.inner.write();
        *inner = Inner::new(new_epoch);
    }
}
