// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    authority_batch::{BatchSender, BroadcastReceiver, BroadcastSender},
    execution_engine,
};
use move_binary_format::CompiledModule;
use move_bytecode_utils::module_cache::ModuleCache;
use move_core_types::{
    language_storage::{ModuleId, StructTag},
    resolver::{ModuleResolver, ResourceResolver},
};
use move_vm_runtime::native_functions::NativeFunctionTable;
use std::sync::atomic::AtomicUsize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    pin::Pin,
    sync::Arc,
};
use sui_adapter::adapter;
use sui_types::{
    base_types::*,
    batch::UpdateItem,
    committee::Committee,
    crypto::AuthoritySignature,
    error::{SuiError, SuiResult},
    fp_bail, fp_ensure, gas,
    messages::*,
    object::{Data, Object, Owner},
    storage::{BackingPackageStore, DeleteKind, Storage},
    MOVE_STDLIB_ADDRESS, SUI_FRAMEWORK_ADDRESS,
};
use tracing::*;

#[cfg(test)]
#[path = "unit_tests/authority_tests.rs"]
pub mod authority_tests;

#[cfg(test)]
#[path = "unit_tests/batch_transaction_tests.rs"]
mod batch_transaction_tests;

#[cfg(test)]
#[path = "unit_tests/move_integration_tests.rs"]
pub mod move_integration_tests;

mod temporary_store;
pub use temporary_store::AuthorityTemporaryStore;

mod authority_store;
pub use authority_store::AuthorityStore;

// based on https://github.com/diem/move/blob/62d48ce0d8f439faa83d05a4f5cd568d4bfcb325/language/tools/move-cli/src/sandbox/utils/mod.rs#L50
const MAX_GAS_BUDGET: u64 = 18446744073709551615 / 1000 - 1;
const MAX_ITEMS_LIMIT: u64 = 10_000;

/// a Trait object for `signature::Signer` that is:
/// - Pin, i.e. confined to one place in memory (we don't want to copy private keys).
/// - Sync, i.e. can be safely shared between threads.
///
/// Typically instantiated with Box::pin(keypair) where keypair is a `KeyPair`
///
pub type StableSyncAuthoritySigner =
    Pin<Arc<dyn signature::Signer<AuthoritySignature> + Send + Sync>>;

pub struct AuthorityState {
    // Fixed size, static, identity of the authority
    /// The name of this authority.
    pub name: AuthorityName,
    /// Committee of this Sui instance.
    pub committee: Committee,
    /// The signature key of the authority.
    pub secret: StableSyncAuthoritySigner,

    /// Move native functions that are available to invoke
    _native_functions: NativeFunctionTable,
    move_vm: Arc<adapter::MoveVM>,

    /// The database
    _database: Arc<AuthorityStore>,

    // Structures needed for handling batching and notifications.
    /// The sender to notify of new transactions
    /// and create batches for this authority.
    /// Keep as None if there is no need for this.
    batch_channels: Option<(BatchSender, BroadcastSender)>,

    /// Ensures there can only be a single consensus client is updating the state.
    pub consensus_guardrail: AtomicUsize,
}

/// The authority state encapsulates all state, drives execution, and ensures safety.
///
/// Note the authority operations can be accessed through a read ref (&) and do not
/// require &mut. Internally a database is synchronized through a mutex lock.
///
/// Repeating valid commands should produce no changes and return no error.
impl AuthorityState {
    /// Set a listener for transaction certificate updates. Returns an
    /// error if a listener is already registered.
    pub fn set_batch_sender(
        &mut self,
        batch_sender: BatchSender,
        broadcast_sender: BroadcastSender,
    ) -> SuiResult {
        if self.batch_channels.is_some() {
            return Err(SuiError::AuthorityUpdateFailure);
        }
        self.batch_channels = Some((batch_sender, broadcast_sender));
        Ok(())
    }

    /// Get a broadcast receiver for updates
    pub fn subscribe(&self) -> Result<BroadcastReceiver, SuiError> {
        self.batch_channels
            .as_ref()
            .map(|(_, tx)| tx.subscribe())
            .ok_or_else(|| SuiError::GenericAuthorityError {
                error: "No broadcast subscriptions allowed for this authority.".to_string(),
            })
    }

    /// The logic to check one object against a reference, and return the object if all is well
    /// or an error if not.
    fn check_one_lock(
        &self,
        transaction: &Transaction,
        object_kind: InputObjectKind,
        object: &Object,
        owned_object_authenticators: &HashSet<SuiAddress>,
    ) -> SuiResult {
        match object_kind {
            InputObjectKind::MovePackage(package_id) => {
                fp_ensure!(
                    object.data.try_as_package().is_some(),
                    SuiError::MoveObjectAsPackage {
                        object_id: package_id
                    }
                );
            }
            InputObjectKind::OwnedMoveObject((object_id, sequence_number, object_digest)) => {
                fp_ensure!(
                    sequence_number <= SequenceNumber::MAX,
                    SuiError::InvalidSequenceNumber
                );

                // Check that the seq number is the same
                fp_ensure!(
                    object.version() == sequence_number,
                    SuiError::UnexpectedSequenceNumber {
                        object_id,
                        expected_sequence: object.version(),
                        given_sequence: sequence_number,
                    }
                );

                // Check the digest matches
                fp_ensure!(
                    object.digest() == object_digest,
                    SuiError::InvalidObjectDigest {
                        object_id,
                        expected_digest: object_digest
                    }
                );

                match object.owner {
                    Owner::SharedImmutable => {
                        // Nothing else to check for SharedImmutable.
                    }
                    Owner::AddressOwner(owner) => {
                        // Check the owner is the transaction sender.
                        fp_ensure!(
                            transaction.sender_address() == owner,
                            SuiError::IncorrectSigner {
                                error: format!("Object {:?} is owned by account address {:?}, but signer address is {:?}", object.id(), owner, transaction.sender_address()),
                            }
                        );
                    }
                    Owner::ObjectOwner(owner) => {
                        // Check that the object owner is another mutable object in the input.
                        fp_ensure!(
                            owned_object_authenticators.contains(&owner),
                            SuiError::IncorrectSigner {
                                error: format!("Object {:?} is owned by object {:?}, which is not in the input", object.id(), owner),
                            }
                        );
                    }
                    Owner::SharedMutable => {
                        // This object is a mutable shared object. However the transaction
                        // specifies it as an owned object. This is inconsistent.
                        return Err(SuiError::NotSharedObjectError);
                    }
                };
            }
            InputObjectKind::SharedMoveObject(..) => {
                // When someone locks an object as shared it must be shared already.
                fp_ensure!(object.is_shared(), SuiError::NotSharedObjectError);
            }
        };
        Ok(())
    }

    /// This function does 3 things:
    /// 1. Check if the gas object has enough balance to pay for this transaction.
    ///   Since the transaction may be a batch transaction, we need to walk through
    ///   each single transaction in it and accumulate their gas cost. For Move call
    ///   and publish we can simply use their budget, for transfer we will calculate
    ///   the cost on the spot since it's deterministic (See comments inside the function).
    /// 2. Check if the gas budget for each single transction is above some minimum amount.
    ///   This can help reduce DDos attacks.
    /// 3. Check that the objects used in transfers are mutable. We put the check here
    ///   because this is the most convenient spot to check.
    fn check_gas_requirement(
        transaction: &Transaction,
        input_objects: &[(InputObjectKind, Object)],
    ) -> SuiResult {
        let mut total_cost = 0;
        let mut idx = 0;
        for tx in transaction.single_transactions() {
            match tx {
                SingleTransactionKind::Transfer(_) => {
                    // Index access safe because the inputs were constructed in order.
                    let transfer_object = &input_objects[idx].1;
                    fp_ensure!(
                        !transfer_object.is_read_only(),
                        SuiError::TransferImmutableError
                    );
                    // TODO: Make Transfer transaction to also contain gas_budget.
                    // By @gdanezis: Now his is the only part of this function that requires
                    // an input object besides the gas object. It would be a major win if we
                    // can get rid of the requirement to have all objects to check the transfer
                    // requirement. If we can go this, then we could execute this check before
                    // we check for signatures.
                    // This would allow us to shore up out DoS defences: we only need to do a
                    // read on the gas object balance before we do anything expensive,
                    // such as checking signatures.
                    total_cost += gas::calculate_object_transfer_cost(transfer_object);
                    idx += tx.input_object_count();
                }
                SingleTransactionKind::Call(op) => {
                    gas::check_move_gas_requirement(op.gas_budget)?;
                    total_cost += op.gas_budget;
                    idx += tx.input_object_count();
                }
                SingleTransactionKind::Publish(op) => {
                    gas::check_move_gas_requirement(op.gas_budget)?;
                    total_cost += op.gas_budget;
                    // No need to update idx because Publish cannot show up in batch.
                }
            }
        }
        // The last element in the inputs is always gas object.
        let gas_object = &input_objects.last().unwrap().1;
        gas::check_gas_balance(gas_object, total_cost)
    }

    /// Check all the objects used in the transaction against the database, and ensure
    /// that they are all the correct version and number.
    async fn check_locks(
        &self,
        transaction: &Transaction,
    ) -> Result<Vec<(InputObjectKind, Object)>, SuiError> {
        let input_objects = transaction.input_objects()?;

        // These IDs act as authenticators that can own other objects.
        let objects = self.fetch_objects(&input_objects).await?;

        // Constructing the list of objects that could be used to authenticate other
        // objects. Any mutable object (either shared or owned) can be used to
        // authenticate other objects. Hence essentially we are building the list
        // of mutable objects.
        // We require that mutable objects cannot show up more than once.
        // In [`SingleTransactionKind::input_objects`] we checked that there is no
        // duplicate objects in the same SingleTransactionKind. However for a Batch
        // Transaction, we still need to make sure that the same mutable object don't show
        // up in more than one SingleTransactionKind.
        // TODO: We should be able to allow the same shared mutable object to show up
        // in more than one SingleTransactionKind. We need to ensure that their
        // version number only increases once at the end of the Batch execution.
        let mut owned_object_authenticators: HashSet<SuiAddress> = HashSet::new();
        for object in objects.iter().flatten() {
            if !object.is_read_only() {
                fp_ensure!(
                    owned_object_authenticators.insert(object.id().into()),
                    SuiError::InvalidBatchTransaction {
                        error: format!("Mutable object {} cannot appear in more than one single transactions in a batch", object.id()),
                    }
                );
            }
        }

        // Gather all objects and errors.
        let mut all_objects = Vec::with_capacity(input_objects.len());
        let mut errors = Vec::new();
        for (object_kind, object) in input_objects.into_iter().zip(objects) {
            // All objects must exist in the DB.
            let object = match object {
                Some(object) => object,
                None => {
                    errors.push(object_kind.object_not_found_error());
                    continue;
                }
            };
            // Check if the object contents match the type of lock we need for
            // this object.
            match self.check_one_lock(
                transaction,
                object_kind,
                &object,
                &owned_object_authenticators,
            ) {
                Ok(()) => all_objects.push((object_kind, object)),
                Err(e) => {
                    errors.push(e);
                }
            }
        }
        // If any errors with the locks were detected, we return all errors to give the client
        // a chance to update the authority if possible.
        if !errors.is_empty() {
            return Err(SuiError::LockErrors { errors });
        }
        fp_ensure!(!all_objects.is_empty(), SuiError::ObjectInputArityViolation);
        Self::check_gas_requirement(transaction, &all_objects)?;
        Ok(all_objects)
    }

    async fn fetch_objects(
        &self,
        input_objects: &[InputObjectKind],
    ) -> Result<Vec<Option<Object>>, SuiError> {
        let ids: Vec<_> = input_objects.iter().map(|kind| kind.object_id()).collect();

        self.get_objects(&ids[..]).await
    }

    /// Initiate a new transaction.
    pub async fn handle_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<TransactionInfoResponse, SuiError> {
        // Check the sender's signature.
        transaction.check_signature()?;
        let transaction_digest = transaction.digest();

        // Ensure an idempotent answer.
        if self
            ._database
            .signed_transaction_exists(&transaction_digest)?
        {
            let transaction_info = self.make_transaction_info(&transaction_digest).await?;
            return Ok(transaction_info);
        }

        let owned_objects: Vec<_> = self
            .check_locks(&transaction)
            .instrument(tracing::trace_span!("tx_check_locks"))
            .await?
            .into_iter()
            .filter_map(|(object_kind, object)| match object_kind {
                InputObjectKind::MovePackage(_) => None,
                InputObjectKind::OwnedMoveObject(object_ref) => {
                    if object.is_read_only() {
                        None
                    } else {
                        Some(object_ref)
                    }
                }
                InputObjectKind::SharedMoveObject(..) => None,
            })
            .collect();

        debug!(
            num_mutable_objects = owned_objects.len(),
            "Checked locks and found mutable objects"
        );

        let signed_transaction = SignedTransaction::new(transaction, self.name, &*self.secret);

        // Check and write locks, to signed transaction, into the database
        // The call to self.set_transaction_lock checks the lock is not conflicting,
        // and returns ConflictingTransaction error in case there is a lock on a different
        // existing transaction.
        self.set_transaction_lock(&owned_objects, signed_transaction)
            .instrument(tracing::trace_span!("db_set_transaction_lock"))
            .await?;

        // Return the signed Transaction or maybe a cert.
        self.make_transaction_info(&transaction_digest).await
    }

    /// Confirm a transfer.
    pub async fn handle_confirmation_transaction(
        &self,
        confirmation_transaction: ConfirmationTransaction,
    ) -> SuiResult<TransactionInfoResponse> {
        let transaction_digest = *confirmation_transaction.certificate.digest();

        // Ensure an idempotent answer.
        if self._database.signed_effects_exists(&transaction_digest)? {
            let transaction_info = self.make_transaction_info(&transaction_digest).await?;
            return Ok(transaction_info);
        }

        // Check the certificate and retrieve the transfer data.
        confirmation_transaction
            .certificate
            .check(&self.committee)?;

        self.process_certificate(confirmation_transaction).await
    }

    async fn check_shared_locks(
        &self,
        transaction_digest: &TransactionDigest,
        transaction: &Transaction,
        inputs: &[(InputObjectKind, Object)],
    ) -> Result<(), SuiError> {
        // If the transaction contains shared objects, we need to ensure they have been scheduled
        // for processing by the consensus protocol.
        if transaction.contains_shared_object() {
            debug!("Validating shared object sequence numbers from consensus...");

            // Collect the version we have for each shared object
            let shared_ids: HashSet<_> = inputs
                .iter()
                .filter_map(|(kind, obj)| match kind {
                    InputObjectKind::SharedMoveObject(..) if obj.owner.is_shared_mutable() => {
                        Some((obj.id(), obj.version()))
                    }
                    _ => None,
                })
                .collect();
            // Internal consistency check
            debug_assert!(
                !shared_ids.is_empty(),
                "we just checked that there are share objects yet none found?"
            );

            // Read the
            let shared_locks: HashMap<_, _> = self
                ._database
                .all_shared_locks(transaction_digest)?
                .into_iter()
                .collect();

            // Check whether the shared objects have already been assigned a sequence number by
            // the consensus. Bail if the transaction contains even one shared object that either:
            // (i) was not assigned a sequence number, or
            // (ii) has a different sequence number than the current one.

            let lock_errors: Vec<_> = shared_ids
                .iter()
                .filter_map(|(object_id, version)| {
                    if !shared_locks.contains_key(object_id) {
                        Some(SuiError::SharedObjectLockNotSetObject)
                    } else if shared_locks[object_id] != *version {
                        Some(SuiError::UnexpectedSequenceNumber {
                            object_id: *object_id,
                            expected_sequence: shared_locks[object_id],
                            given_sequence: *version,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            fp_ensure!(
                lock_errors.is_empty(),
                SuiError::LockErrors {
                    errors: lock_errors
                }
            );
        }

        Ok(())
    }

    async fn process_certificate(
        &self,
        confirmation_transaction: ConfirmationTransaction,
    ) -> Result<TransactionInfoResponse, SuiError> {
        let certificate = confirmation_transaction.certificate;
        let transaction_digest = *certificate.digest();
        let transaction = &certificate.transaction;

        let objects_by_kind = self.check_locks(transaction).await?;

        // At this point we need to check if any shared objects need locks,
        // and whether they have them.
        let _shared_objects = self
            .check_shared_locks(&transaction_digest, transaction, &objects_by_kind)
            .await?;
        // inputs.extend(shared_objects);

        debug!(
            num_inputs = objects_by_kind.len(),
            "Read inputs for transaction from DB"
        );

        let mut transaction_dependencies: BTreeSet<_> = objects_by_kind
            .iter()
            .map(|(_, object)| object.previous_transaction)
            .collect();

        // Insert into the certificates map
        let mut tx_ctx = TxContext::new(&transaction.sender_address(), &transaction_digest);

        let gas_object_id = transaction.gas_payment_object_ref().0;
        let mut temporary_store =
            AuthorityTemporaryStore::new(self._database.clone(), &objects_by_kind, tx_ctx.digest());
        let status = execution_engine::execute_transaction(
            &mut temporary_store,
            transaction.clone(),
            objects_by_kind,
            &mut tx_ctx,
            &self.move_vm,
            self._native_functions.clone(),
        )?;
        debug!(
            gas_used = status.gas_used(),
            "Finished execution of transaction with status {:?}", status
        );

        // Remove from dependencies the generic hash
        transaction_dependencies.remove(&TransactionDigest::genesis());

        let signed_effects = temporary_store.to_signed_effects(
            &self.name,
            &*self.secret,
            &transaction_digest,
            transaction_dependencies.into_iter().collect(),
            status,
            &gas_object_id,
        );
        // Update the database in an atomic manner
        let (seq, resp) = self
            .update_state(temporary_store, certificate, signed_effects)
            .instrument(tracing::debug_span!("db_update_state"))
            .await?; // Returns the OrderInfoResponse

        // If there is a notifier registered, notify:
        if let Some((sender, _)) = &self.batch_channels {
            sender.send_item(seq, transaction_digest).await?;
        }

        Ok(resp)
    }

    /// Process certificates coming from the consensus. It is crucial that this function is only
    /// called by a single task (ie. the task handling consensus outputs).
    pub async fn handle_consensus_certificate(
        &self,
        certificate: CertifiedTransaction,
        last_consensus_index: SequenceNumber,
    ) -> SuiResult<()> {
        // Ensure it is a shared object certificate
        if !certificate.transaction.contains_shared_object() {
            log::debug!(
                "Transaction without shared object has been sequenced: {:?}",
                certificate.transaction
            );
            return Ok(());
        }

        // Ensure it is the first time we see this certificate.
        let transaction_digest = *certificate.digest();
        if self._database.sequenced(
            &transaction_digest,
            certificate.transaction.shared_input_objects(),
        )?[0]
            .is_some()
        {
            return Ok(());
        }

        // Check the certificate.
        certificate.check(&self.committee)?;

        // Persist the certificate since we are about to lock one or more shared object.
        // We thus need to make sure someone (if not the client) can continue the protocol.
        // Also atomically lock the shared objects for this particular transaction and
        // increment the last consensus index. Note that a single process can ever call
        // this function and that the last consensus index is also kept in memory. It is
        // thus ok to only persist now (despite this function may have returned earlier).
        // In the worst case, the synchronizer of the consensus client will catch up.
        self._database
            .persist_certificate_and_lock_shared_objects(certificate, last_consensus_index)
    }

    pub async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<TransactionInfoResponse, SuiError> {
        self.make_transaction_info(&request.transaction_digest)
            .await
    }

    pub async fn handle_account_info_request(
        &self,
        request: AccountInfoRequest,
    ) -> Result<AccountInfoResponse, SuiError> {
        self.make_account_info(request.account)
    }

    pub async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<ObjectInfoResponse, SuiError> {
        let ref_and_digest = match request.request_kind {
            ObjectInfoRequestKind::PastObjectInfo(seq) => {
                // Get the Transaction Digest that created the object
                let parent_iterator = self
                    .get_parent_iterator(request.object_id, Some(seq))
                    .await?;

                parent_iterator
                    .first()
                    .map(|(object_ref, tx_digest)| (*object_ref, *tx_digest))
            }
            ObjectInfoRequestKind::LatestObjectInfo(_) => {
                // Or get the latest object_reference and transaction entry.
                self.get_latest_parent_entry(request.object_id).await?
            }
        };

        let (requested_object_reference, parent_certificate) = match ref_and_digest {
            Some((object_ref, transaction_digest)) => (
                Some(object_ref),
                if transaction_digest == TransactionDigest::genesis() {
                    None
                } else {
                    // Get the cert from the transaction digest
                    Some(self.read_certificate(&transaction_digest).await?.ok_or(
                        SuiError::CertificateNotfound {
                            certificate_digest: transaction_digest,
                        },
                    )?)
                },
            ),
            None => (None, None),
        };

        // Return the latest version of the object and the current lock if any, if requested.
        let object_and_lock = match request.request_kind {
            ObjectInfoRequestKind::LatestObjectInfo(request_layout) => {
                match self.get_object(&request.object_id).await {
                    Ok(Some(object)) => {
                        let lock = if object.is_read_only() {
                            // Read only objects have no locks.
                            None
                        } else {
                            self.get_transaction_lock(&object.compute_object_reference())
                                .await?
                        };
                        let layout = match request_layout {
                            Some(format) => {
                                let resolver = ModuleCache::new(&self);
                                object.get_layout(format, &resolver)?
                            }
                            None => None,
                        };

                        Some(ObjectResponse {
                            object,
                            lock,
                            layout,
                        })
                    }
                    Err(e) => return Err(e),
                    _ => None,
                }
            }
            ObjectInfoRequestKind::PastObjectInfo(_) => None,
        };

        Ok(ObjectInfoResponse {
            parent_certificate,
            requested_object_reference,
            object_and_lock,
        })
    }

    /// Handles a request for a batch info. It returns a sequence of
    /// [batches, transactions, batches, transactions] as UpdateItems, and a flag
    /// that if true indicates the request goes beyond the last batch in the
    /// database.
    pub async fn handle_batch_info_request(
        &self,
        request: BatchInfoRequest,
    ) -> Result<(VecDeque<UpdateItem>, bool), SuiError> {
        // Ensure the range contains some elements and end > start
        if request.end <= request.start {
            return Err(SuiError::InvalidSequenceRangeError);
        };

        // Ensure we are not doing too much work per request
        if request.end - request.start > MAX_ITEMS_LIMIT {
            return Err(SuiError::TooManyItemsError(MAX_ITEMS_LIMIT));
        }

        let (batches, transactions) = self
            ._database
            .batches_and_transactions(request.start, request.end)?;

        let mut dq_batches = std::collections::VecDeque::from(batches);
        let mut dq_transactions = std::collections::VecDeque::from(transactions);
        let mut items = VecDeque::with_capacity(dq_batches.len() + dq_transactions.len());
        let mut last_batch_next_seq = 0;

        // Send full historical data as [Batch - Transactions - Batch - Transactions - Batch].
        while let Some(current_batch) = dq_batches.pop_front() {
            // Get all transactions belonging to this batch and send them
            loop {
                // No more items or item too large for this batch
                if dq_transactions.is_empty()
                    || dq_transactions[0].0 >= current_batch.batch.next_sequence_number
                {
                    break;
                }

                let current_transaction = dq_transactions.pop_front().unwrap();
                items.push_back(UpdateItem::Transaction(current_transaction));
            }

            // Now send the batch
            last_batch_next_seq = current_batch.batch.next_sequence_number;
            items.push_back(UpdateItem::Batch(current_batch));
        }

        // whether we have sent everything requested, or need to start
        // live notifications.
        let should_subscribe = request.end > last_batch_next_seq;

        // If any transactions are left they must be outside a batch
        while let Some(current_transaction) = dq_transactions.pop_front() {
            // Remember the last sequence sent
            items.push_back(UpdateItem::Transaction(current_transaction));
        }

        Ok((items, should_subscribe))
    }

    pub async fn new(
        committee: Committee,
        name: AuthorityName,
        secret: StableSyncAuthoritySigner,
        store: Arc<AuthorityStore>,
        genesis_packages: Vec<Vec<CompiledModule>>,
        genesis_ctx: &mut TxContext,
    ) -> Self {
        let state = AuthorityState::new_without_genesis(committee, name, secret, store).await;

        for genesis_modules in genesis_packages {
            state
                .store_package_and_init_modules_for_genesis(genesis_ctx, genesis_modules)
                .await
                .expect("We expect publishing the Genesis packages to not fail");
        }
        state
    }

    pub async fn new_without_genesis(
        committee: Committee,
        name: AuthorityName,
        secret: StableSyncAuthoritySigner,
        store: Arc<AuthorityStore>,
    ) -> Self {
        let native_functions =
            sui_framework::natives::all_natives(MOVE_STDLIB_ADDRESS, SUI_FRAMEWORK_ADDRESS);

        Self {
            committee,
            name,
            secret,
            _native_functions: native_functions.clone(),
            move_vm: adapter::new_move_vm(native_functions)
                .expect("We defined natives to not fail here"),
            _database: store,
            batch_channels: None,
            consensus_guardrail: AtomicUsize::new(0),
        }
    }

    pub(crate) fn db(&self) -> Arc<AuthorityStore> {
        self._database.clone()
    }

    #[cfg(test)]
    pub(crate) fn batch_sender(&self) -> &BatchSender {
        &self.batch_channels.as_ref().unwrap().0
    }

    async fn get_object(&self, object_id: &ObjectID) -> Result<Option<Object>, SuiError> {
        self._database.get_object(object_id)
    }

    pub async fn insert_object(&self, object: Object) {
        self._database
            .insert_object(object)
            .expect("TODO: propagate the error")
    }

    /// Persist the Genesis package to DB along with the side effects for module initialization
    async fn store_package_and_init_modules_for_genesis(
        &self,
        ctx: &mut TxContext,
        modules: Vec<CompiledModule>,
    ) -> SuiResult {
        debug_assert!(ctx.digest() == TransactionDigest::genesis());
        let inputs = Transaction::input_objects_in_compiled_modules(&modules);
        let input_objects = self.fetch_objects(&inputs).await?;
        // When publishing genesis packages, since the std framework packages all have
        // non-zero addresses, [`Transaction::input_objects_in_compiled_modules`] will consider
        // them as dependencies even though they are not. Hence input_objects contain objects
        // that don't exist on-chain because they are yet to be published.
        #[cfg(debug_assertions)]
        {
            let to_be_published_addresses: HashSet<_> = modules
                .iter()
                .map(|module| *module.self_id().address())
                .collect();
            assert!(
                // An object either exists on-chain, or is one of the packages to be published.
                inputs
                    .iter()
                    .zip(input_objects.iter())
                    .all(|(kind, obj_opt)| obj_opt.is_some()
                        || to_be_published_addresses.contains(&kind.object_id()))
            );
        }
        let filtered = inputs
            .into_iter()
            .zip(input_objects.into_iter())
            .filter_map(|(input, object_opt)| object_opt.map(|object| (input, object)))
            .collect::<Vec<_>>();

        let mut temporary_store =
            AuthorityTemporaryStore::new(self._database.clone(), &filtered, ctx.digest());
        let package_id = ObjectID::from(*modules[0].self_id().address());
        let natives = self._native_functions.clone();
        let vm = adapter::verify_and_link(&temporary_store, &modules, package_id, natives)?;
        if let ExecutionStatus::Failure { error, .. } = adapter::store_package_and_init_modules(
            &mut temporary_store,
            &vm,
            modules,
            ctx,
            MAX_GAS_BUDGET,
        ) {
            return Err(*error);
        };
        self._database
            .update_objects_state_for_genesis(temporary_store, ctx.digest())
    }

    /// Make an information response for a transaction
    async fn make_transaction_info(
        &self,
        transaction_digest: &TransactionDigest,
    ) -> Result<TransactionInfoResponse, SuiError> {
        self._database.get_transaction_info(transaction_digest)
    }

    fn make_account_info(&self, account: SuiAddress) -> Result<AccountInfoResponse, SuiError> {
        self._database
            .get_account_objects(account)
            .map(|object_ids| AccountInfoResponse {
                object_ids,
                owner: account,
            })
    }

    // Helper function to manage transaction_locks

    /// Set the transaction lock to a specific transaction
    pub async fn set_transaction_lock(
        &self,
        mutable_input_objects: &[ObjectRef],
        signed_transaction: SignedTransaction,
    ) -> Result<(), SuiError> {
        self._database
            .set_transaction_lock(mutable_input_objects, signed_transaction)
    }

    async fn update_state(
        &self,
        temporary_store: AuthorityTemporaryStore<AuthorityStore>,
        certificate: CertifiedTransaction,
        signed_effects: SignedTransactionEffects,
    ) -> Result<(u64, TransactionInfoResponse), SuiError> {
        self._database
            .update_state(temporary_store, certificate, signed_effects)
    }

    /// Get a read reference to an object/seq lock
    pub async fn get_transaction_lock(
        &self,
        object_ref: &ObjectRef,
    ) -> Result<Option<SignedTransaction>, SuiError> {
        self._database.get_transaction_lock(object_ref)
    }

    // Helper functions to manage certificates

    /// Read from the DB of certificates
    pub async fn read_certificate(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<CertifiedTransaction>, SuiError> {
        self._database.read_certificate(digest)
    }

    pub async fn parent(&self, object_ref: &ObjectRef) -> Option<TransactionDigest> {
        self._database
            .parent(object_ref)
            .expect("TODO: propagate the error")
    }

    pub async fn get_objects(
        &self,
        _objects: &[ObjectID],
    ) -> Result<Vec<Option<Object>>, SuiError> {
        self._database.get_objects(_objects)
    }

    /// Returns all parents (object_ref and transaction digests) that match an object_id, at
    /// any object version, or optionally at a specific version.
    pub async fn get_parent_iterator(
        &self,
        object_id: ObjectID,
        seq: Option<SequenceNumber>,
    ) -> Result<Vec<(ObjectRef, TransactionDigest)>, SuiError> {
        {
            self._database.get_parent_iterator(object_id, seq)
        }
    }

    pub async fn get_latest_parent_entry(
        &self,
        object_id: ObjectID,
    ) -> Result<Option<(ObjectRef, TransactionDigest)>, SuiError> {
        self._database.get_latest_parent_entry(object_id)
    }

    pub fn last_consensus_index(&self) -> SuiResult<SequenceNumber> {
        self._database.last_consensus_index()
    }
}

impl ModuleResolver for AuthorityState {
    type Error = SuiError;

    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error> {
        self._database.get_module(module_id)
    }
}
