// Copyright (c) Facebook, Inc. and its affiliates.
// SPDX-License-Identifier: Apache-2.0

use crate::{base_types::*, committee::Committee, error::FastPayError, messages::*};
use std::{collections::BTreeMap, convert::TryInto};

#[cfg(test)]
#[path = "unit_tests/authority_tests.rs"]
mod authority_tests;

#[derive(Eq, PartialEq, Debug)]
pub struct ObjectState {
    /// The object identifier
    pub id: ObjectID,
    /// The authenticator that unlocks this object (eg. public key, or other).
    pub owner: FastPayAddress,
    /// Sequence number tracking spending actions.
    pub next_sequence_number: SequenceNumber,
    /// The contents of the Object. Right now just a blob.
    pub contents: Vec<u8>,

    // These structures will likely be refactored outside the object.
    /// Whether we have signed a transfer for this sequence number already.
    pub pending_confirmation_legacy: Option<SignedTransferOrder>,
    /// All confirmed certificates for this sender.
    pub confirmed_log: Vec<CertifiedTransferOrder>,
}

pub struct AuthorityState {
    // Fixed size, static, identity of the authority and shard
    /// The name of this authority.
    pub name: AuthorityName,
    /// Committee of this FastPay instance.
    pub committee: Committee,
    /// The signature key of the authority.
    pub secret: KeyPair,
    /// The sharding ID of this authority shard. 0 if one shard.
    pub shard_id: ShardId,
    /// The number of shards. 1 if single shard.
    pub number_of_shards: u32,

    // The variable length dynamic state of the authority shard
    /// States of fastnft objects
    objects: BTreeMap<ObjectID, ObjectState>,
    /// Order lock map maps object versions to the first next transaction seen
    order_lock: BTreeMap<(ObjectID, SequenceNumber), Option<SignedTransferOrder>>,
}

/// Interface provided by each (shard of an) authority.
/// All commands return either the current account info or an error.
/// Repeating commands produces no changes and returns no error.
pub trait Authority {
    /// Initiate a new transfer to a FastPay or Primary account.
    fn handle_transfer_order(
        &mut self,
        order: TransferOrder,
    ) -> Result<AccountInfoResponse, FastPayError>;

    /// Confirm a transfer to a FastPay or Primary account.
    fn handle_confirmation_order(
        &mut self,
        order: ConfirmationOrder,
    ) -> Result<AccountInfoResponse, FastPayError>;

    /// Handle information requests for this account.
    fn handle_account_info_request(
        &self,
        request: AccountInfoRequest,
    ) -> Result<AccountInfoResponse, FastPayError>;
}

impl Authority for AuthorityState {
    /// Initiate a new transfer.
    fn handle_transfer_order(
        &mut self,
        order: TransferOrder,
    ) -> Result<AccountInfoResponse, FastPayError> {
        // Check the sender's signature and retrieve the transfer data.
        fp_ensure!(
            self.in_shard(&order.transfer.object_id),
            FastPayError::WrongShard
        );
        order.check_signature()?;
        let transfer = &order.transfer;
        let object_id = transfer.object_id;
        fp_ensure!(
            transfer.sequence_number <= SequenceNumber::max(),
            FastPayError::InvalidSequenceNumber
        );

        match self.objects.get_mut(&object_id) {
            None => fp_bail!(FastPayError::UnknownSenderAccount),
            Some(object) => {
                // Check the transaction sender is also the object owner
                fp_ensure!(
                    order.transfer.sender == object.owner,
                    FastPayError::IncorrectSigner
                );

                if let Some(pending_confirmation) = &object.pending_confirmation_legacy {
                    fp_ensure!(
                        &pending_confirmation.value.transfer == transfer,
                        FastPayError::PreviousTransferMustBeConfirmedFirst {
                            pending_confirmation: pending_confirmation.value.clone()
                        }
                    );
                    // This exact transfer order was already signed. Return the previous value.
                    return Ok(object.make_account_info());
                }
                fp_ensure!(
                    object.next_sequence_number == transfer.sequence_number,
                    FastPayError::UnexpectedSequenceNumber
                );

                let signed_order = SignedTransferOrder::new(order, self.name, &self.secret);
                object.pending_confirmation_legacy = Some(signed_order.clone());
                let info = object.make_account_info();

                // Add to the order_lock structure.
                self.set_order_lock(signed_order)?;

                Ok(info)
            }
        }
    }

    /// Confirm a transfer.
    fn handle_confirmation_order(
        &mut self,
        confirmation_order: ConfirmationOrder,
    ) -> Result<AccountInfoResponse, FastPayError> {
        let certificate = confirmation_order.transfer_certificate;
        // Check the certificate and retrieve the transfer data.
        fp_ensure!(
            self.in_shard(&certificate.value.transfer.object_id),
            FastPayError::WrongShard
        );
        certificate.check(&self.committee)?;
        let transfer = certificate.value.transfer.clone();

        // If we have a certificate on the confirmation order it means that the input
        // object exists on other honest authorities, but we do not have it. The only
        // way this may happen is if we missed some updates.
        if !self.objects.contains_key(&transfer.object_id) {
            fp_bail!(FastPayError::MissingEalierConfirmations {
                current_sequence_number: SequenceNumber::from(0),
            });
        }

        // First we copy all relevant data from sender.
        let mut sender_object = self
            .objects
            .entry(transfer.object_id)
            .or_insert_with(ObjectState::new);

        let mut sender_sequence_number = sender_object.next_sequence_number;

        // Check and update the copied state
        if sender_sequence_number < transfer.sequence_number {
            fp_bail!(FastPayError::MissingEalierConfirmations {
                current_sequence_number: sender_sequence_number
            });
        }
        if sender_sequence_number > transfer.sequence_number {
            // Transfer was already confirmed.
            return Ok(sender_object.make_account_info());
        }
        sender_sequence_number = sender_sequence_number.increment()?;

        // Commit sender state back to the database (Must never fail!)
        sender_object.id = transfer.object_id;
        sender_object.owner = match transfer.recipient {
            Address::Primary(_) => PublicKeyBytes([0; 32]),
            Address::FastPay(addr) => addr,
        };

        sender_object.next_sequence_number = sender_sequence_number;
        sender_object.pending_confirmation_legacy = None;
        sender_object.confirmed_log.push(certificate);
        let info = sender_object.make_account_info();

        // Init the order lock for this object
        self.init_order_lock(transfer.object_id, sender_sequence_number)?;

        Ok(info)
    }

    fn handle_account_info_request(
        &self,
        request: AccountInfoRequest,
    ) -> Result<AccountInfoResponse, FastPayError> {
        fp_ensure!(self.in_shard(&request.object_id), FastPayError::WrongShard);
        let account = self.object_state(&request.object_id)?;
        let mut response = account.make_account_info();
        if let Some(seq) = request.request_sequence_number {
            if let Some(cert) = account.confirmed_log.get(usize::from(seq)) {
                response.requested_certificate = Some(cert.clone());
            } else {
                fp_bail!(FastPayError::CertificateNotfound)
            }
        }
        Ok(response)
    }
}

impl Default for ObjectState {
    fn default() -> Self {
        Self {
            id: [0; 20],
            contents: Vec::new(),
            owner: PublicKeyBytes([0; 32]),
            next_sequence_number: SequenceNumber::new(),
            pending_confirmation_legacy: None,
            confirmed_log: Vec::new(),
        }
    }
}

impl ObjectState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_object_reference(&self) -> ObjectRef {
        (self.id, self.next_sequence_number)
    }

    fn make_account_info(&self) -> AccountInfoResponse {
        AccountInfoResponse {
            object_id: self.id,
            owner: self.owner,
            next_sequence_number: self.next_sequence_number,
            pending_confirmation: self.pending_confirmation_legacy.clone(),
            requested_certificate: None,
            requested_received_transfers: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn new_with_balance(contents: Vec<u8>, _received_log: Vec<CertifiedTransferOrder>) -> Self {
        Self {
            id: [0; 20],
            owner: PublicKeyBytes([0; 32]),
            contents,
            next_sequence_number: SequenceNumber::new(),
            pending_confirmation_legacy: None,
            confirmed_log: Vec::new(),
        }
    }
}

impl AuthorityState {
    pub fn new(committee: Committee, name: AuthorityName, secret: KeyPair) -> Self {
        AuthorityState {
            committee,
            name,
            secret,
            objects: BTreeMap::new(),
            order_lock: BTreeMap::new(),
            shard_id: 0,
            number_of_shards: 1,
        }
    }

    pub fn new_shard(
        committee: Committee,
        name: AuthorityName,
        secret: KeyPair,
        shard_id: u32,
        number_of_shards: u32,
    ) -> Self {
        AuthorityState {
            committee,
            name,
            secret,
            objects: BTreeMap::new(),
            order_lock: BTreeMap::new(),
            shard_id,
            number_of_shards,
        }
    }

    pub fn in_shard(&self, object_id: &ObjectID) -> bool {
        self.which_shard(object_id) == self.shard_id
    }

    pub fn get_shard(num_shards: u32, object_id: &ObjectID) -> u32 {
        const LAST_INTEGER_INDEX: usize = std::mem::size_of::<ObjectID>() - 4;
        u32::from_le_bytes(object_id[LAST_INTEGER_INDEX..].try_into().expect("4 bytes"))
            % num_shards
    }

    pub fn which_shard(&self, object_id: &ObjectID) -> u32 {
        Self::get_shard(self.number_of_shards, object_id)
    }

    fn object_state(&self, object_id: &ObjectID) -> Result<&ObjectState, FastPayError> {
        self.objects
            .get(object_id)
            .ok_or(FastPayError::UnknownSenderAccount)
    }

    pub fn insert_object(&mut self, object: ObjectState) {
        self.objects.insert(object.id, object);
    }

    #[cfg(test)]
    pub fn accounts_mut(&mut self) -> &mut BTreeMap<ObjectID, ObjectState> {
        &mut self.objects
    }

    // Helper function to manage order_locks

    /// Initialize an order lock for an object/sequence to None
    pub fn init_order_lock(
        &mut self,
        object_id: ObjectID,
        seq: SequenceNumber,
    ) -> Result<(), FastPayError> {
        if self.order_lock.contains_key(&(object_id, seq)) {
            return Err(FastPayError::OrderLockExists);
        }

        self.order_lock.insert((object_id, seq), None);
        Ok(())
    }

    /// Set the order lock to a specific transaction
    pub fn set_order_lock(
        &mut self,
        signed_order: SignedTransferOrder,
    ) -> Result<(), FastPayError> {
        let object_id = signed_order.value.transfer.object_id;
        let seq = signed_order.value.transfer.sequence_number;

        // The object / version must exist, and therefore lock initialized.
        if !self.order_lock.contains_key(&(object_id, seq)) {
            return Err(FastPayError::OrderLockDoesNotExist);
        }

        // Note: Safe to unwrap thanks to the contains_key check above.
        let lock = self.order_lock.get_mut(&(object_id, seq)).unwrap();
        if let Some(_existing_signed_order) = lock {
            if _existing_signed_order.value.transfer == signed_order.value.transfer {
                // For some reason we are re-inserting the same order. Not optimal but correct.
                return Ok(());
            } else {
                // We are trying to set the lock to a different order, this is unsafe.
                return Err(FastPayError::OrderLockReset);
            }
        }

        // The lock is None, so we replace it with the given order.
        lock.replace(signed_order);
        Ok(())
    }

    /// Get a read reference to an object/seq lock
    pub fn get_order_lock(
        &self,
        object_id: ObjectID,
        seq: SequenceNumber,
    ) -> Result<&Option<SignedTransferOrder>, FastPayError> {
        // The object / version must exist, and therefore lock initialized.
        if !self.order_lock.contains_key(&(object_id, seq)) {
            return Err(FastPayError::OrderLockDoesNotExist);
        }

        // Note: Safe to unwrap thanks to the contains_key check above.
        let lock = self.order_lock.get(&(object_id, seq)).unwrap();
        Ok(lock)
    }
}
