// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::authority::AuthorityStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sui_types::base_types::*;
use sui_types::error::{SuiError, SuiResult};

use std::collections::BTreeMap;
use std::time::Duration;
use sui_types::crypto::{sha3_hash, AuthoritySignature, BcsSignable, Signature};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::interval;

use typed_store::Map;

#[cfg(test)]
#[path = "unit_tests/batch_tests.rs"]
mod batch_tests;

/*

An authority asynchronously creates batches from its sequence of
certificates / effects. Then both the sequence of certificates
/ effects are transmitted to listeners (as a transaction digest)
as well as batches.

The architecture is as follows:
- The authority store notifies through the Sender that a new
  certificate / effect has been sequenced, at a specific sequence
  number.
- The sender sends this information through a channel to the Manager,
  that decides whether a new batch should be made. This is based on
  time elapsed as well as current size of batch. If so a new batch
  is created.
- The authority manager also holds the sending ends of a number of
  channels that eventually go to clients that registered interest
  in receiving all updates from the authority. When a new item is
  sequenced of a batch created this is sent out to them.

*/

pub type TxSequenceNumber = u64;

pub type BroadcastPair = (
    tokio::sync::broadcast::Sender<UpdateItem>,
    tokio::sync::broadcast::Receiver<UpdateItem>,
);

/// Either a freshly sequenced transaction hash or a batch
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Hash, Debug, Serialize, Deserialize)]
pub enum UpdateItem {
    Transaction((TxSequenceNumber, TransactionDigest)),
    Batch(AuthorityBatch),
}

pub struct BatchSender {
    /// Channel for sending updates.
    tx_send: Sender<(TxSequenceNumber, TransactionDigest)>,
}

pub struct BatchManager {
    /// Channel for receiving updates
    tx_recv: Receiver<(TxSequenceNumber, TransactionDigest)>,
    /// The sender end of the broadcast channel used to send updates to listeners
    tx_broadcast: tokio::sync::broadcast::Sender<UpdateItem>,
    /// Copy of the database to write batches and read transactions.
    db: Arc<AuthorityStore>,
}

impl BatchSender {
    /// Send a new event to the batch manager
    pub async fn send_item(
        &self,
        transaction_sequence: TxSequenceNumber,
        transaction_digest: TransactionDigest,
    ) -> Result<(), SuiError> {
        self.tx_send
            .send((transaction_sequence, transaction_digest))
            .await
            .map_err(|_| SuiError::BatchErrorSender)
    }
}

impl BatchManager {
    pub fn new(
        db: Arc<AuthorityStore>,
        capacity: usize,
    ) -> (BatchSender, BatchManager, BroadcastPair) {
        let (tx_send, tx_recv) = channel(capacity);
        let (tx_broadcast, rx_broadcast) = tokio::sync::broadcast::channel(capacity);
        let sender = BatchSender { tx_send };
        let manager = BatchManager {
            tx_recv,
            tx_broadcast: tx_broadcast.clone(),
            db,
        };

        (sender, manager, (tx_broadcast, rx_broadcast))
    }

    /// Starts the manager service / tokio task
    pub async fn start_service(
        mut self,
        min_batch_size: u64,
        max_delay: Duration,
    ) -> Result<tokio::task::JoinHandle<()>, SuiError> {
        let last_batch = self.init_from_database().await?;

        let join_handle = tokio::spawn(async move {
            self.run_service(last_batch, min_batch_size, max_delay)
                .await
                .expect("Service returns with no errors");
            drop(self);
        });

        Ok(join_handle)
    }

    async fn init_from_database(&self) -> Result<AuthorityBatch, SuiError> {
        // First read the last batch in the db
        let mut last_batch = match self
            .db
            .batches
            .iter()
            .skip_prior_to(&TxSequenceNumber::MAX)?
            .next()
        {
            Some((_, last_batch)) => last_batch,
            None => {
                // Make a batch at zero
                let zero_batch = AuthorityBatch::initial();
                self.db.batches.insert(&0, &zero_batch)?;
                zero_batch
            }
        };

        // See if there are any transactions in the database not in a batch
        let mut total_seq = self
            .db
            .next_sequence_number
            .load(std::sync::atomic::Ordering::Relaxed);
        if total_seq > last_batch.total_size {
            // Make a new batch, to put the old transactions not in a batch in.
            let mut transactions: Vec<_> = self
                .db
                .executed_sequence
                .iter()
                .skip_to(&last_batch.total_size)?
                .collect();

            if transactions.len() as TxSequenceNumber != total_seq - last_batch.total_size {
                /*
                NOTE: The database is corrupt, namely we have a higher maximum transaction sequence
                      than number of items since the end of the last batch,
                      which means there is a hole in the sequence. This can happen
                      in case we crash after writting command seq x but before x-1 was written.
                */
                let db_batch = self.db.executed_sequence.batch();

                // Delete the transactions that we read above, that are out of a batch and we are about
                // to re-sequence with new numbers.
                let db_batch = db_batch.delete_batch(
                    &self.db.executed_sequence,
                    transactions.iter().map(|(k, _)| *k),
                )?;

                // Reorder the transactions
                total_seq = last_batch.total_size + transactions.len() as TxSequenceNumber;
                self.db
                    .next_sequence_number
                    .store(total_seq, std::sync::atomic::Ordering::Relaxed);

                /*
                Update transactions

                Here we re-write the transactions in the same order but using a new sequential
                sequence number (the range) to ensure the sequence of transactions contains no
                gaps in the sequence.
                */
                let range = last_batch.total_size..total_seq;

                transactions = range
                    .into_iter()
                    .zip(transactions.into_iter().map(|(_, v)| v))
                    .collect();

                let db_batch = db_batch
                    .insert_batch(&self.db.executed_sequence, transactions.iter().cloned())?;

                db_batch.write()?;
            }

            last_batch = AuthorityBatch::make_next(&last_batch, &transactions[..]);
            self.db.batches.insert(&total_seq, &last_batch)?;
        }

        Ok(last_batch)
    }

    pub async fn run_service(
        &mut self,
        prev_batch: AuthorityBatch,
        min_batch_size: u64,
        max_delay: Duration,
    ) -> SuiResult {
        // Then we operate in a loop, where for each new update we consider
        // whether to create a new batch or not.

        let mut interval = interval(max_delay);
        let mut exit = false;
        let mut make_batch;

        let mut prev_batch = prev_batch;

        // The structures we use to build the next batch. The current_batch holds the sequence
        // of transactions in order, following the last batch. The loose transactions holds
        // transactions we may have received out of order.
        let mut current_batch: Vec<(TxSequenceNumber, TransactionDigest)> = Vec::new();
        let mut loose_transactions: BTreeMap<TxSequenceNumber, TransactionDigest> = BTreeMap::new();

        let mut next_sequence_number = prev_batch.total_size;

        while !exit {
            // Reset the flags.
            make_batch = false;

            // check if we should make a new block
            tokio::select! {
              _ = interval.tick() => {
                // Every so often we check if we should make a batch
                // but it should never be empty. But never empty.
                  make_batch = true;
              },
              item_option = self.tx_recv.recv() => {

                match item_option {
                  None => {
                    make_batch = true;
                    exit = true;
                  },
                  Some((seq, tx_digest)) => {
                    loose_transactions.insert(seq, tx_digest);
                    while loose_transactions.contains_key(&next_sequence_number) {
                      let next_item = (next_sequence_number, loose_transactions.remove(&next_sequence_number).unwrap());
                      // Send the update
                      let _ = self.tx_broadcast.send(UpdateItem::Transaction(next_item));
                      current_batch.push(next_item);
                      next_sequence_number += 1;
                    }

                    if current_batch.len() as TxSequenceNumber >= min_batch_size {
                      make_batch = true;
                    }
                  }
                }
               }
            }

            // Logic to make a batch
            if make_batch {
                if current_batch.is_empty() {
                    continue;
                }

                // Make and store a new batch.
                let new_batch = AuthorityBatch::make_next(&prev_batch, &current_batch);
                self.db.batches.insert(&new_batch.total_size, &new_batch)?;

                // Send the update
                let _ = self.tx_broadcast.send(UpdateItem::Batch(new_batch.clone()));

                // A new batch is actually made, so we reset the conditions.
                prev_batch = new_batch;
                current_batch.clear();

                // We rest the interval here to ensure that blocks
                // are made either when they are full or old enough.
                interval.reset();
            }
        }

        // When a new batch is created we send a notification to all who have
        // registered an interest.

        Ok(())
    }

    /// Register a sending channel used to send streaming
    /// updates to clients.
    pub fn register_listener() {}
}

pub type BatchDigest = [u8; 32];

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Hash, Default, Debug, Serialize, Deserialize)]
pub struct TransactionBatch(Vec<(TxSequenceNumber, TransactionDigest)>);
impl BcsSignable for TransactionBatch {}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Hash, Default, Debug, Serialize, Deserialize)]
pub struct AuthorityBatch {
    /// The total number of items executed by this authority.
    total_size: u64,

    /// The number of items in the previous block.
    previous_total_size: u64,

    /// The digest of the previous block, if there is one
    previous_digest: Option<BatchDigest>,

    // The digest of all transactions digests in this batch
    transactions_digest: [u8; 32],
}

impl BcsSignable for AuthorityBatch {}

impl AuthorityBatch {
    pub fn digest(&self) -> BatchDigest {
        sha3_hash(self)
    }

    /// The first batch for any authority indexes at zero
    /// and has zero length.
    pub fn initial() -> AuthorityBatch {
        let to_hash = TransactionBatch(Vec::new());
        let transactions_digest = sha3_hash(&to_hash);

        AuthorityBatch {
            total_size: 0,
            previous_total_size: 0,
            previous_digest: None,
            transactions_digest,
        }
    }

    /// Make a batch, containing some transactions, and following the previous
    /// batch.
    pub fn make_next(
        previous_batch: &AuthorityBatch,
        transactions: &[(TxSequenceNumber, TransactionDigest)],
    ) -> AuthorityBatch {
        let to_hash = TransactionBatch(transactions.to_vec());
        let transactions_digest = sha3_hash(&to_hash);

        AuthorityBatch {
            total_size: previous_batch.total_size + transactions.len() as u64,
            previous_total_size: previous_batch.total_size,
            previous_digest: Some(previous_batch.digest()),
            transactions_digest,
        }
    }
}
