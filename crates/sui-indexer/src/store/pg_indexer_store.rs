// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any as StdAny;
use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use core::result::Result::Ok;
use diesel::dsl::{max, min};
use diesel::r2d2::R2D2Connection;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::{QueryDsl, RunQueryDsl};
use downcast::Any;
use itertools::Itertools;
use tap::TapFallible;
use tracing::info;

use sui_protocol_config::ProtocolConfig;
use sui_types::base_types::ObjectID;

use crate::db::ConnectionPool;
use crate::errors::{Context, IndexerError};
use crate::handlers::EpochToCommit;
use crate::handlers::TransactionObjectChangesToCommit;
use crate::metrics::IndexerMetrics;
use crate::models::checkpoints::StoredChainIdentifier;
use crate::models::checkpoints::StoredCheckpoint;
use crate::models::checkpoints::StoredCpTx;
use crate::models::display::StoredDisplay;
use crate::models::epoch::StoredEpochInfo;
use crate::models::epoch::{StoredFeatureFlag, StoredProtocolConfig};
use crate::models::events::StoredEvent;
use crate::models::obj_indices::StoredObjectVersion;
use crate::models::objects::{
    StoredDeletedHistoryObject, StoredDeletedObject, StoredHistoryObject, StoredObject,
    StoredObjectSnapshot,
};
use crate::models::packages::StoredPackage;
use crate::models::transactions::StoredTransaction;
use crate::schema::{
    chain_identifier, checkpoints, display, epochs, event_emit_module, event_emit_package,
    event_senders, event_struct_instantiation, event_struct_module, event_struct_name,
    event_struct_package, events, feature_flags, objects, objects_history, objects_snapshot, objects_version,
    packages, protocol_configs, pruner_cp_watermark, transactions, tx_calls_fun, tx_calls_mod, tx_calls_pkg,
    tx_changed_objects, tx_digests, tx_input_objects, tx_kinds, tx_recipients, tx_senders,
};
use crate::types::EventIndex;
use crate::types::{IndexedCheckpoint, IndexedEvent, IndexedPackage, IndexedTransaction, TxIndex};
use crate::{
    insert_or_ignore_into, on_conflict_do_update, persist_chunk_into_table, read_only_blocking,
    transactional_blocking_with_retry,
};

use super::pg_partition_manager::{EpochPartitionData, PgPartitionManager};
use super::IndexerStore;
use super::ObjectChangeToCommit;

#[cfg(feature = "postgres-feature")]
use diesel::upsert::excluded;
use sui_types::digests::{ChainIdentifier, CheckpointDigest};

#[macro_export]
macro_rules! chunk {
    ($data: expr, $size: expr) => {{
        $data
            .into_iter()
            .chunks($size)
            .into_iter()
            .map(|c| c.collect())
            .collect::<Vec<Vec<_>>>()
    }};
}

macro_rules! prune_tx_or_event_indice_table {
    ($table:ident, $conn:expr, $min_tx:expr, $max_tx:expr, $context_msg:expr) => {
        diesel::delete($table::table.filter($table::tx_sequence_number.between($min_tx, $max_tx)))
            .execute($conn)
            .map_err(IndexerError::from)
            .context($context_msg)?;
    };
}

// In one DB transaction, the update could be chunked into
// a few statements, this is the amount of rows to update in one statement
// TODO: I think with the `per_db_tx` params, `PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX`
// is now less relevant. We should do experiments and remove it if it's true.
const PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX: usize = 1000;
// The amount of rows to update in one DB transaction
const PG_COMMIT_PARALLEL_CHUNK_SIZE: usize = 100;
// The amount of rows to update in one DB transaction, for objects particularly
// Having this number too high may cause many db deadlocks because of
// optimistic locking.
const PG_COMMIT_OBJECTS_PARALLEL_CHUNK_SIZE: usize = 500;
const PG_DB_COMMIT_SLEEP_DURATION: Duration = Duration::from_secs(3600);

// with rn = 1, we only select the latest version of each object,
// so that we don't have to update the same object multiple times.
const UPDATE_OBJECTS_SNAPSHOT_QUERY: &str = r"
INSERT INTO objects_snapshot (object_id, object_version, object_status, object_digest, checkpoint_sequence_number, owner_type, owner_id, object_type, object_type_package, object_type_module, object_type_name, serialized_object, coin_type, coin_balance, df_kind, df_name, df_object_type, df_object_id)
SELECT object_id, object_version, object_status, object_digest, checkpoint_sequence_number, owner_type, owner_id, object_type, object_type_package, object_type_module, object_type_name, serialized_object, coin_type, coin_balance, df_kind, df_name, df_object_type, df_object_id
FROM (
    SELECT *,
           ROW_NUMBER() OVER (PARTITION BY object_id ORDER BY object_version DESC) as rn
    FROM objects_history
    WHERE checkpoint_sequence_number >= $1 AND checkpoint_sequence_number < $2
) as subquery
WHERE rn = 1
ON CONFLICT (object_id) DO UPDATE
SET object_version = EXCLUDED.object_version,
    object_status = EXCLUDED.object_status,
    object_digest = EXCLUDED.object_digest,
    checkpoint_sequence_number = EXCLUDED.checkpoint_sequence_number,
    owner_type = EXCLUDED.owner_type,
    owner_id = EXCLUDED.owner_id,
    object_type = EXCLUDED.object_type,
    object_type_package = EXCLUDED.object_type_package,
    object_type_module = EXCLUDED.object_type_module,
    object_type_name = EXCLUDED.object_type_name,
    serialized_object = EXCLUDED.serialized_object,
    coin_type = EXCLUDED.coin_type,
    coin_balance = EXCLUDED.coin_balance,
    df_kind = EXCLUDED.df_kind,
    df_name = EXCLUDED.df_name,
    df_object_type = EXCLUDED.df_object_type,
    df_object_id = EXCLUDED.df_object_id;
";

#[derive(Clone)]
pub struct PgIndexerStoreConfig {
    pub parallel_chunk_size: usize,
    pub parallel_objects_chunk_size: usize,
    pub epochs_to_keep: Option<u64>,
}

pub struct PgIndexerStore<T: R2D2Connection + 'static> {
    blocking_cp: ConnectionPool<T>,
    metrics: IndexerMetrics,
    partition_manager: PgPartitionManager<T>,
    config: PgIndexerStoreConfig,
}

impl<T: R2D2Connection> Clone for PgIndexerStore<T> {
    fn clone(&self) -> PgIndexerStore<T> {
        Self {
            blocking_cp: self.blocking_cp.clone(),
            metrics: self.metrics.clone(),
            partition_manager: self.partition_manager.clone(),
            config: self.config.clone(),
        }
    }
}

impl<T: R2D2Connection + 'static> PgIndexerStore<T> {
    pub fn new(blocking_cp: ConnectionPool<T>, metrics: IndexerMetrics) -> Self {
        let parallel_chunk_size = std::env::var("PG_COMMIT_PARALLEL_CHUNK_SIZE")
            .unwrap_or_else(|_e| PG_COMMIT_PARALLEL_CHUNK_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        let parallel_objects_chunk_size = std::env::var("PG_COMMIT_OBJECTS_PARALLEL_CHUNK_SIZE")
            .unwrap_or_else(|_e| PG_COMMIT_OBJECTS_PARALLEL_CHUNK_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        let epochs_to_keep = std::env::var("EPOCHS_TO_KEEP")
            .map(|s| s.parse::<u64>().ok())
            .unwrap_or_else(|_e| None);
        let partition_manager = PgPartitionManager::new(blocking_cp.clone())
            .expect("Failed to initialize partition manager");
        let config = PgIndexerStoreConfig {
            parallel_chunk_size,
            parallel_objects_chunk_size,
            epochs_to_keep,
        };

        Self {
            blocking_cp,
            metrics,
            partition_manager,
            config,
        }
    }

    pub fn blocking_cp(&self) -> ConnectionPool<T> {
        self.blocking_cp.clone()
    }

    pub fn get_latest_epoch_id(&self) -> Result<Option<u64>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select(max(epochs::epoch))
                .first::<Option<i64>>(conn)
                .map(|v| v.map(|v| v as u64))
        })
        .context("Failed reading latest epoch id from PostgresDB")
    }

    /// Get the range of the protocol versions that need to be indexed.
    pub fn get_protocol_version_index_range(&self) -> Result<(i64, i64), IndexerError> {
        // We start indexing from the next protocol version after the latest one stored in the db.
        let start = read_only_blocking!(&self.blocking_cp, |conn| {
            protocol_configs::dsl::protocol_configs
                .select(max(protocol_configs::protocol_version))
                .first::<Option<i64>>(conn)
        })
        .context("Failed reading latest protocol version from PostgresDB")?
        .map_or(1, |v| v + 1);

        // We end indexing at the protocol version of the latest epoch stored in the db.
        let end = read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select(max(epochs::protocol_version))
                .first::<Option<i64>>(conn)
        })
        .context("Failed reading latest epoch protocol version from PostgresDB")?
        .unwrap_or(1);
        Ok((start, end))
    }

    pub fn get_chain_identifier(&self) -> Result<Option<Vec<u8>>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            chain_identifier::dsl::chain_identifier
                .select(chain_identifier::checkpoint_digest)
                .first::<Vec<u8>>(conn)
                .optional()
        })
        .context("Failed reading chain id from PostgresDB")
    }

    fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::dsl::checkpoints
                .select(max(checkpoints::sequence_number))
                .first::<Option<i64>>(conn)
                .map(|v| v.map(|v| v as u64))
        })
        .context("Failed reading latest checkpoint sequence number from PostgresDB")
    }

    fn get_available_checkpoint_range(&self) -> Result<(u64, u64), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::dsl::checkpoints
                .select((
                    min(checkpoints::sequence_number),
                    max(checkpoints::sequence_number),
                ))
                .first::<(Option<i64>, Option<i64>)>(conn)
                .map(|(min, max)| {
                    (
                        min.unwrap_or_default() as u64,
                        max.unwrap_or_default() as u64,
                    )
                })
        })
        .context("Failed reading min and max checkpoint sequence numbers from PostgresDB")
    }

    fn get_prunable_epoch_range(&self) -> Result<(u64, u64), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select((min(epochs::epoch), max(epochs::epoch)))
                .first::<(Option<i64>, Option<i64>)>(conn)
                .map(|(min, max)| {
                    (
                        min.unwrap_or_default() as u64,
                        max.unwrap_or_default() as u64,
                    )
                })
        })
        .context("Failed reading min and max epoch numbers from PostgresDB")
    }

    fn get_min_prunable_checkpoint(&self) -> Result<u64, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            pruner_cp_watermark::dsl::pruner_cp_watermark
                .select(min(pruner_cp_watermark::checkpoint_sequence_number))
                .first::<Option<i64>>(conn)
                .map(|v| v.unwrap_or_default() as u64)
        })
        .context("Failed reading min prunable checkpoint sequence number from PostgresDB")
    }

    fn get_checkpoint_range_for_epoch(
        &self,
        epoch: u64,
    ) -> Result<(u64, Option<u64>), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select((epochs::first_checkpoint_id, epochs::last_checkpoint_id))
                .filter(epochs::epoch.eq(epoch as i64))
                .first::<(i64, Option<i64>)>(conn)
                .map(|(min, max)| (min as u64, max.map(|v| v as u64)))
        })
        .context("Failed reading checkpoint range from PostgresDB")
    }

    fn get_transaction_range_for_checkpoint(
        &self,
        checkpoint: u64,
    ) -> Result<(u64, u64), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            pruner_cp_watermark::dsl::pruner_cp_watermark
                .select((
                    pruner_cp_watermark::min_tx_sequence_number,
                    pruner_cp_watermark::max_tx_sequence_number,
                ))
                .filter(pruner_cp_watermark::checkpoint_sequence_number.eq(checkpoint as i64))
                .first::<(i64, i64)>(conn)
                .map(|(min, max)| (min as u64, max as u64))
        })
        .context("Failed reading transaction range from PostgresDB")
    }

    fn get_latest_object_snapshot_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<u64>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            objects_snapshot::dsl::objects_snapshot
                .select(max(objects_snapshot::checkpoint_sequence_number))
                .first::<Option<i64>>(conn)
                .map(|v| v.map(|v| v as u64))
        })
        .context("Failed reading latest object snapshot checkpoint sequence number from PostgresDB")
    }

    fn persist_display_updates(
        &self,
        display_updates: BTreeMap<String, StoredDisplay>,
    ) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                on_conflict_do_update!(
                    display::table,
                    display_updates.values().collect::<Vec<_>>(),
                    display::object_type,
                    (
                        display::id.eq(excluded(display::id)),
                        display::version.eq(excluded(display::version)),
                        display::bcs.eq(excluded(display::bcs)),
                    ),
                    |excluded: &StoredDisplay| (
                        display::id.eq(excluded.id.clone()),
                        display::version.eq(excluded.version),
                        display::bcs.eq(excluded.bcs.clone()),
                    ),
                    conn
                );
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )?;

        Ok(())
    }

    fn persist_object_mutation_chunk(
        &self,
        mutated_object_mutation_chunk: Vec<StoredObject>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_chunks
            .start_timer();
        let len = mutated_object_mutation_chunk.len();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                on_conflict_do_update!(
                    objects::table,
                    mutated_object_mutation_chunk.clone(),
                    objects::object_id,
                    (
                        objects::object_id.eq(excluded(objects::object_id)),
                        objects::object_version.eq(excluded(objects::object_version)),
                        objects::object_digest.eq(excluded(objects::object_digest)),
                        objects::checkpoint_sequence_number
                            .eq(excluded(objects::checkpoint_sequence_number)),
                        objects::owner_type.eq(excluded(objects::owner_type)),
                        objects::owner_id.eq(excluded(objects::owner_id)),
                        objects::object_type.eq(excluded(objects::object_type)),
                        objects::serialized_object.eq(excluded(objects::serialized_object)),
                        objects::coin_type.eq(excluded(objects::coin_type)),
                        objects::coin_balance.eq(excluded(objects::coin_balance)),
                        objects::df_kind.eq(excluded(objects::df_kind)),
                        objects::df_name.eq(excluded(objects::df_name)),
                        objects::df_object_type.eq(excluded(objects::df_object_type)),
                        objects::df_object_id.eq(excluded(objects::df_object_id)),
                    ),
                    |excluded: StoredObject| (
                        objects::object_id.eq(excluded.object_id.clone()),
                        objects::object_version.eq(excluded.object_version),
                        objects::object_digest.eq(excluded.object_digest.clone()),
                        objects::checkpoint_sequence_number.eq(excluded.checkpoint_sequence_number),
                        objects::owner_type.eq(excluded.owner_type),
                        objects::owner_id.eq(excluded.owner_id.clone()),
                        objects::object_type.eq(excluded.object_type.clone()),
                        objects::serialized_object.eq(excluded.serialized_object.clone()),
                        objects::coin_type.eq(excluded.coin_type.clone()),
                        objects::coin_balance.eq(excluded.coin_balance),
                        objects::df_kind.eq(excluded.df_kind),
                        objects::df_name.eq(excluded.df_name.clone()),
                        objects::df_object_type.eq(excluded.df_object_type.clone()),
                        objects::df_object_id.eq(excluded.df_object_id.clone()),
                    ),
                    conn
                );
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted {} chunked objects", len);
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist object mutations with error: {}", e);
        })
    }

    fn persist_object_deletion_chunk(
        &self,
        deleted_objects_chunk: Vec<StoredDeletedObject>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_chunks
            .start_timer();
        let len = deleted_objects_chunk.len();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::delete(
                    objects::table.filter(
                        objects::object_id.eq_any(
                            deleted_objects_chunk
                                .iter()
                                .map(|o| o.object_id.clone())
                                .collect::<Vec<_>>(),
                        ),
                    ),
                )
                .execute(conn)
                .map_err(IndexerError::from)
                .context("Failed to write object deletion to PostgresDB")?;

                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Deleted {} chunked objects", len);
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist object deletions with error: {}", e);
        })
    }

    fn backfill_objects_snapshot_chunk(
        &self,
        objects: Vec<ObjectChangeToCommit>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_snapshot_chunks
            .start_timer();
        let mut objects_snapshot: Vec<StoredObjectSnapshot> = vec![];
        for object in objects {
            match object {
                ObjectChangeToCommit::MutatedObject(stored_object) => {
                    objects_snapshot.push(stored_object.into());
                }
                ObjectChangeToCommit::DeletedObject(stored_deleted_object) => {
                    objects_snapshot.push(stored_deleted_object.into());
                }
            }
        }

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for objects_snapshot_chunk in
                    objects_snapshot.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    on_conflict_do_update!(
                        objects_snapshot::table,
                        objects_snapshot_chunk,
                        objects_snapshot::object_id,
                        (
                            objects_snapshot::object_version
                                .eq(excluded(objects_snapshot::object_version)),
                            objects_snapshot::object_status
                                .eq(excluded(objects_snapshot::object_status)),
                            objects_snapshot::object_digest
                                .eq(excluded(objects_snapshot::object_digest)),
                            objects_snapshot::checkpoint_sequence_number
                                .eq(excluded(objects_snapshot::checkpoint_sequence_number)),
                            objects_snapshot::owner_type.eq(excluded(objects_snapshot::owner_type)),
                            objects_snapshot::owner_id.eq(excluded(objects_snapshot::owner_id)),
                            objects_snapshot::object_type_package
                                .eq(excluded(objects_snapshot::object_type_package)),
                            objects_snapshot::object_type_module
                                .eq(excluded(objects_snapshot::object_type_module)),
                            objects_snapshot::object_type_name
                                .eq(excluded(objects_snapshot::object_type_name)),
                            objects_snapshot::object_type
                                .eq(excluded(objects_snapshot::object_type)),
                            objects_snapshot::serialized_object
                                .eq(excluded(objects_snapshot::serialized_object)),
                            objects_snapshot::coin_type.eq(excluded(objects_snapshot::coin_type)),
                            objects_snapshot::coin_balance
                                .eq(excluded(objects_snapshot::coin_balance)),
                            objects_snapshot::df_kind.eq(excluded(objects_snapshot::df_kind)),
                            objects_snapshot::df_name.eq(excluded(objects_snapshot::df_name)),
                            objects_snapshot::df_object_type
                                .eq(excluded(objects_snapshot::df_object_type)),
                            objects_snapshot::df_object_id
                                .eq(excluded(objects_snapshot::df_object_id)),
                        ),
                        |excluded: StoredObjectSnapshot| (
                            objects_snapshot::object_version.eq(excluded.object_version),
                            objects_snapshot::object_status.eq(excluded.object_status),
                            objects_snapshot::object_digest.eq(excluded.object_digest),
                            objects_snapshot::checkpoint_sequence_number
                                .eq(excluded.checkpoint_sequence_number),
                            objects_snapshot::owner_type.eq(excluded.owner_type),
                            objects_snapshot::owner_id.eq(excluded.owner_id),
                            objects_snapshot::object_type_package.eq(excluded.object_type_package),
                            objects_snapshot::object_type_module.eq(excluded.object_type_module),
                            objects_snapshot::object_type_name.eq(excluded.object_type_name),
                            objects_snapshot::object_type.eq(excluded.object_type),
                            objects_snapshot::serialized_object.eq(excluded.serialized_object),
                            objects_snapshot::coin_type.eq(excluded.coin_type),
                            objects_snapshot::coin_balance.eq(excluded.coin_balance),
                            objects_snapshot::df_kind.eq(excluded.df_kind),
                            objects_snapshot::df_name.eq(excluded.df_name),
                            objects_snapshot::df_object_type.eq(excluded.df_object_type),
                            objects_snapshot::df_object_id.eq(excluded.df_object_id),
                        ),
                        conn
                    );
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked objects snapshot",
                objects_snapshot.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist object snapshot with error: {}", e);
        })
    }

    fn persist_objects_history_chunk(
        &self,
        objects: Vec<ObjectChangeToCommit>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_history_chunks
            .start_timer();
        let mut mutated_objects: Vec<StoredHistoryObject> = vec![];
        let mut object_versions: Vec<StoredObjectVersion> = vec![];
        let mut deleted_object_ids: Vec<StoredDeletedHistoryObject> = vec![];
        for object in objects {
            match object {
                ObjectChangeToCommit::MutatedObject(stored_object) => {
                    object_versions.push(StoredObjectVersion::from(&stored_object));
                    mutated_objects.push(stored_object.into());
                }
                ObjectChangeToCommit::DeletedObject(stored_deleted_object) => {
                    object_versions.push(StoredObjectVersion::from(&stored_deleted_object));
                    deleted_object_ids.push(stored_deleted_object.into());
                }
            }
        }

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for mutated_object_change_chunk in
                    mutated_objects.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(
                        objects_history::table,
                        mutated_object_change_chunk,
                        conn
                    );
                }

                for object_version_chunk in object_versions.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(objects_version::table, object_version_chunk, conn);
                }

                for deleted_objects_chunk in
                    deleted_object_ids.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(objects_history::table, deleted_objects_chunk, conn);
                }

                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked objects history",
                mutated_objects.len() + deleted_object_ids.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist object history with error: {}", e);
        })
    }

    fn update_objects_snapshot(&self, start_cp: u64, end_cp: u64) -> Result<(), IndexerError> {
        let work_mem_gb = std::env::var("INDEXER_PG_WORK_MEM")
            .unwrap_or_else(|_e| "16".to_string())
            .parse::<i64>()
            .unwrap();
        let pg_work_mem_query_string = format!("SET work_mem = '{}GB'", work_mem_gb);
        let pg_work_mem_query = pg_work_mem_query_string.as_str();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| { RunQueryDsl::execute(diesel::sql_query(pg_work_mem_query), conn,) },
            PG_DB_COMMIT_SLEEP_DURATION
        )?;

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                RunQueryDsl::execute(
                    diesel::sql_query(UPDATE_OBJECTS_SNAPSHOT_QUERY)
                        .bind::<diesel::sql_types::BigInt, _>(start_cp as i64)
                        .bind::<diesel::sql_types::BigInt, _>(end_cp as i64),
                    conn,
                )
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )?;
        Ok(())
    }

    fn persist_checkpoints(&self, checkpoints: Vec<IndexedCheckpoint>) -> Result<(), IndexerError> {
        let Some(first_checkpoint) = checkpoints.first() else {
            return Ok(());
        };

        // If the first checkpoint has sequence number 0, we need to persist the digest as
        // chain identifier.
        if first_checkpoint.sequence_number == 0 {
            let checkpoint_digest = first_checkpoint.checkpoint_digest.into_inner().to_vec();
            self.persist_protocol_configs_and_feature_flags(checkpoint_digest.clone())?;
            transactional_blocking_with_retry!(
                &self.blocking_cp,
                |conn| {
                    let checkpoint_digest =
                        first_checkpoint.checkpoint_digest.into_inner().to_vec();
                    insert_or_ignore_into!(
                        chain_identifier::table,
                        StoredChainIdentifier { checkpoint_digest },
                        conn
                    );
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )?;
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_checkpoints
            .start_timer();

        let stored_cp_txs = checkpoints.iter().map(StoredCpTx::from).collect::<Vec<_>>();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for stored_cp_tx_chunk in stored_cp_txs.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(pruner_cp_watermark::table, stored_cp_tx_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            info!(
                "Persisted {} pruner_cp_watermark rows.",
                stored_cp_txs.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist pruner_cp_watermark with error: {}", e);
        })?;

        let stored_checkpoints = checkpoints
            .iter()
            .map(StoredCheckpoint::from)
            .collect::<Vec<_>>();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for stored_checkpoint_chunk in
                    stored_checkpoints.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(checkpoints::table, stored_checkpoint_chunk, conn);
                    let time_now_ms = chrono::Utc::now().timestamp_millis();
                    for stored_checkpoint in stored_checkpoint_chunk {
                        self.metrics
                            .db_commit_lag_ms
                            .set(time_now_ms - stored_checkpoint.timestamp_ms);
                        self.metrics.max_committed_checkpoint_sequence_number.set(
                            stored_checkpoint.sequence_number,
                        );
                        self.metrics.committed_checkpoint_timestamp_ms.set(
                            stored_checkpoint.timestamp_ms,
                        );
                    }
                    for stored_checkpoint in stored_checkpoint_chunk {
                        info!("Indexer lag: persisted checkpoint {} with time now {} and checkpoint time {}", stored_checkpoint.sequence_number, time_now_ms, stored_checkpoint.timestamp_ms);
                    }
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} checkpoints",
                stored_checkpoints.len()
            );
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist checkpoints with error: {}", e);
        })
    }

    fn persist_transactions_chunk(
        &self,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_transactions_chunks
            .start_timer();
        let transformation_guard = self
            .metrics
            .checkpoint_db_commit_latency_transactions_chunks_transformation
            .start_timer();
        let transactions = transactions
            .iter()
            .map(StoredTransaction::from)
            .collect::<Vec<_>>();
        drop(transformation_guard);

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for transaction_chunk in transactions.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(transactions::table, transaction_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked transactions",
                transactions.len()
            );
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist transactions with error: {}", e);
        })
    }

    fn persist_events_chunk(&self, events: Vec<IndexedEvent>) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_events_chunks
            .start_timer();
        let len = events.len();
        let events = events
            .into_iter()
            .map(StoredEvent::from)
            .collect::<Vec<_>>();

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for event_chunk in events.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(events::table, event_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted {} chunked events", len);
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist events with error: {}", e);
        })
    }

    fn persist_packages(&self, packages: Vec<IndexedPackage>) -> Result<(), IndexerError> {
        if packages.is_empty() {
            return Ok(());
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_packages
            .start_timer();
        let packages = packages
            .into_iter()
            .map(StoredPackage::from)
            .collect::<Vec<_>>();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for packages_chunk in packages.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    on_conflict_do_update!(
                        packages::table,
                        packages_chunk,
                        packages::package_id,
                        (
                            packages::package_id.eq(excluded(packages::package_id)),
                            packages::move_package.eq(excluded(packages::move_package)),
                        ),
                        |excluded: StoredPackage| (
                            packages::package_id.eq(excluded.package_id.clone()),
                            packages::move_package.eq(excluded.move_package),
                        ),
                        conn
                    );
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted {} packages", packages.len());
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist packages with error: {}", e);
        })
    }

    async fn persist_event_indices_chunk(
        &self,
        indices: Vec<EventIndex>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_event_indices_chunks
            .start_timer();
        let len = indices.len();
        let (
            event_emit_packages,
            event_emit_modules,
            event_senders,
            event_struct_packages,
            event_struct_modules,
            event_struct_names,
            event_struct_instantiations,
        ) = indices.into_iter().map(|i| i.split()).fold(
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            |(
                mut event_emit_packages,
                mut event_emit_modules,
                mut event_senders,
                mut event_struct_packages,
                mut event_struct_modules,
                mut event_struct_names,
                mut event_struct_instantiations,
            ),
             index| {
                event_emit_packages.push(index.0);
                event_emit_modules.push(index.1);
                event_senders.push(index.2);
                event_struct_packages.push(index.3);
                event_struct_modules.push(index.4);
                event_struct_names.push(index.5);
                event_struct_instantiations.push(index.6);
                (
                    event_emit_packages,
                    event_emit_modules,
                    event_senders,
                    event_struct_packages,
                    event_struct_modules,
                    event_struct_names,
                    event_struct_instantiations,
                )
            },
        );

        // Now persist all the event indices in parallel into their tables.
        let mut futures = vec![];
        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_emit_package::table,
                event_emit_packages,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_emit_module::table,
                event_emit_modules,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(event_senders::table, event_senders, &this.blocking_cp)
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_package::table,
                event_struct_packages,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_module::table,
                event_struct_modules,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_name::table,
                event_struct_names,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_instantiation::table,
                event_struct_instantiations,
                &this.blocking_cp
            )
        }));

        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to join event indices futures in a chunk: {}", e);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all event indices in a chunk: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} chunked event indices", len);
        Ok(())
    }

    async fn persist_tx_indices_chunk(&self, indices: Vec<TxIndex>) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_indices_chunks
            .start_timer();
        let len = indices.len();
        let (senders, recipients, input_objects, changed_objects, pkgs, mods, funs, digests, kinds) =
            indices.into_iter().map(|i| i.split()).fold(
                (
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                |(
                    mut tx_senders,
                    mut tx_recipients,
                    mut tx_input_objects,
                    mut tx_changed_objects,
                    mut tx_pkgs,
                    mut tx_mods,
                    mut tx_funs,
                    mut tx_digests,
                    mut tx_kinds,
                ),
                 index| {
                    tx_senders.extend(index.0);
                    tx_recipients.extend(index.1);
                    tx_input_objects.extend(index.2);
                    tx_changed_objects.extend(index.3);
                    tx_pkgs.extend(index.4);
                    tx_mods.extend(index.5);
                    tx_funs.extend(index.6);
                    tx_digests.extend(index.7);
                    tx_kinds.extend(index.8);
                    (
                        tx_senders,
                        tx_recipients,
                        tx_input_objects,
                        tx_changed_objects,
                        tx_pkgs,
                        tx_mods,
                        tx_funs,
                        tx_digests,
                        tx_kinds,
                    )
                },
            );

        let mut futures = vec![];
        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let senders_len = senders.len();
            let recipients_len = recipients.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in senders.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_senders::table, chunk, conn);
                    }
                    for chunk in recipients.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_recipients::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(
                    elapsed,
                    "Persisted {} rows to tx_senders and {} rows to tx_recipients",
                    senders_len,
                    recipients_len,
                );
            })
            .tap_err(|e| {
                tracing::error!(
                    "Failed to persist tx_senders and tx_recipients with error: {}",
                    e
                );
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let input_objects_len = input_objects.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in input_objects.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_input_objects::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(
                    elapsed,
                    "Persisted {} rows to tx_input_objects", input_objects_len,
                );
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_input_objects with error: {}", e);
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let changed_objects_len = changed_objects.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in changed_objects.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_changed_objects::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(
                    elapsed,
                    "Persisted {} rows to tx_changed_objects table", changed_objects_len,
                );
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_changed_objects with error: {}", e);
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let rows_len = pkgs.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in pkgs.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_calls_pkg::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(
                    elapsed,
                    "Persisted {} rows to tx_calls_pkg tables", rows_len
                );
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_calls_pkg with error: {}", e);
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let rows_len = mods.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in mods.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_calls_mod::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(elapsed, "Persisted {} rows to tx_calls_mod table", rows_len);
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_calls_mod with error: {}", e);
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let rows_len = funs.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in funs.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_calls_fun::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(elapsed, "Persisted {} rows to tx_calls_fun table", rows_len);
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_calls_fun with error: {}", e);
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let calls_len = digests.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in digests.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_digests::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                Duration::from_secs(60)
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(elapsed, "Persisted {} rows to tx_digests tables", calls_len);
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_digests with error: {}", e);
            })
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            let now = Instant::now();
            let rows_len = kinds.len();
            transactional_blocking_with_retry!(
                &this.blocking_cp,
                |conn| {
                    for chunk in kinds.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                        insert_or_ignore_into!(tx_kinds::table, chunk, conn);
                    }
                    Ok::<(), IndexerError>(())
                },
                Duration::from_secs(60)
            )
            .tap_ok(|_| {
                let elapsed = now.elapsed().as_secs_f64();
                info!(elapsed, "Persisted {} rows to tx_kinds tables", rows_len);
            })
            .tap_err(|e| {
                tracing::error!("Failed to persist tx_kinds with error: {}", e);
            })
        }));

        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to join tx indices futures in a chunk: {}", e);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all tx indices in a chunk: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} chunked tx_indices", len);
        Ok(())
    }

    fn persist_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_epoch
            .start_timer();
        let epoch_id = epoch.new_epoch.epoch;

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                if let Some(last_epoch) = &epoch.last_epoch {
                    let last_epoch_id = last_epoch.epoch;
                    // Overwrites the `epoch_total_transactions` field on `epoch.last_epoch` because
                    // we are not guaranteed to have the latest data in db when this is set on
                    // indexer's chain-reading side. However, when we `persist_epoch`, the
                    // checkpoints from an epoch ago must have been indexed.
                    let previous_epoch_network_total_transactions = match epoch_id {
                        0 | 1 => 0,
                        _ => {
                            let prev_epoch_id = epoch_id - 2;
                            let result = checkpoints::table
                                .filter(checkpoints::epoch.eq(prev_epoch_id as i64))
                                .select(max(checkpoints::network_total_transactions))
                                .first::<Option<i64>>(conn)
                                .map(|o| o.unwrap_or(0))?;

                            result as u64
                        }
                    };

                    let epoch_total_transactions = epoch.network_total_transactions
                        - previous_epoch_network_total_transactions;

                    let mut last_epoch = StoredEpochInfo::from_epoch_end_info(last_epoch);
                    last_epoch.epoch_total_transactions = Some(epoch_total_transactions as i64);
                    info!(last_epoch_id, "Persisting epoch end data.");
                    on_conflict_do_update!(
                        epochs::table,
                        vec![last_epoch],
                        epochs::epoch,
                        (
                            // Note: Exclude epoch beginning info except system_state below.
                            // This is to ensure that epoch beginning info columns are not overridden with default values,
                            // because these columns are default values in `last_epoch`.
                            epochs::system_state.eq(excluded(epochs::system_state)),
                            epochs::epoch_total_transactions
                                .eq(excluded(epochs::epoch_total_transactions)),
                            epochs::last_checkpoint_id.eq(excluded(epochs::last_checkpoint_id)),
                            epochs::epoch_end_timestamp.eq(excluded(epochs::epoch_end_timestamp)),
                            epochs::storage_fund_reinvestment
                                .eq(excluded(epochs::storage_fund_reinvestment)),
                            epochs::storage_charge.eq(excluded(epochs::storage_charge)),
                            epochs::storage_rebate.eq(excluded(epochs::storage_rebate)),
                            epochs::stake_subsidy_amount.eq(excluded(epochs::stake_subsidy_amount)),
                            epochs::total_gas_fees.eq(excluded(epochs::total_gas_fees)),
                            epochs::total_stake_rewards_distributed
                                .eq(excluded(epochs::total_stake_rewards_distributed)),
                            epochs::leftover_storage_fund_inflow
                                .eq(excluded(epochs::leftover_storage_fund_inflow)),
                            epochs::epoch_commitments.eq(excluded(epochs::epoch_commitments)),
                        ),
                        |excluded: StoredEpochInfo| (
                            epochs::system_state.eq(excluded.system_state.clone()),
                            epochs::epoch_total_transactions.eq(excluded.epoch_total_transactions),
                            epochs::last_checkpoint_id.eq(excluded.last_checkpoint_id),
                            epochs::epoch_end_timestamp.eq(excluded.epoch_end_timestamp),
                            epochs::storage_fund_reinvestment
                                .eq(excluded.storage_fund_reinvestment),
                            epochs::storage_charge.eq(excluded.storage_charge),
                            epochs::storage_rebate.eq(excluded.storage_rebate),
                            epochs::stake_subsidy_amount.eq(excluded.stake_subsidy_amount),
                            epochs::total_gas_fees.eq(excluded.total_gas_fees),
                            epochs::total_stake_rewards_distributed
                                .eq(excluded.total_stake_rewards_distributed),
                            epochs::leftover_storage_fund_inflow
                                .eq(excluded.leftover_storage_fund_inflow),
                            epochs::epoch_commitments.eq(excluded.epoch_commitments)
                        ),
                        conn
                    );
                }

                let epoch_id = epoch.new_epoch.epoch;
                info!(epoch_id, "Persisting epoch beginning info");
                let new_epoch = StoredEpochInfo::from_epoch_beginning_info(&epoch.new_epoch);
                insert_or_ignore_into!(epochs::table, new_epoch, conn);
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, epoch_id, "Persisted epoch beginning info");
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist epoch with error: {}", e);
        })
    }

    fn advance_epoch(&self, epoch_to_commit: EpochToCommit) -> Result<(), IndexerError> {
        let last_epoch_id = epoch_to_commit.last_epoch.as_ref().map(|e| e.epoch);
        // partition_0 has been created, so no need to advance it.
        if let Some(last_epoch_id) = last_epoch_id {
            let last_db_epoch: Option<StoredEpochInfo> =
                read_only_blocking!(&self.blocking_cp, |conn| {
                    epochs::table
                        .filter(epochs::epoch.eq(last_epoch_id as i64))
                        .first::<StoredEpochInfo>(conn)
                        .optional()
                })
                .context("Failed to read last epoch from PostgresDB")?;
            if let Some(last_epoch) = last_db_epoch {
                let epoch_partition_data =
                    EpochPartitionData::compose_data(epoch_to_commit, last_epoch);
                let table_partitions = self.partition_manager.get_table_partitions()?;
                for (table, (_, last_partition)) in table_partitions {
                    // Only advance epoch partition for epoch partitioned tables.
                    if !self
                        .partition_manager
                        .get_strategy(&table)
                        .is_epoch_partitioned()
                    {
                        continue;
                    }
                    let guard = self.metrics.advance_epoch_latency.start_timer();
                    self.partition_manager.advance_epoch(
                        table.clone(),
                        last_partition,
                        &epoch_partition_data,
                    )?;
                    let elapsed = guard.stop_and_record();
                    info!(
                        elapsed,
                        "Advanced epoch partition {} for table {}",
                        last_partition,
                        table.clone()
                    );
                }
            } else {
                tracing::error!("Last epoch: {} from PostgresDB is None.", last_epoch_id);
            }
        }

        Ok(())
    }

    fn prune_checkpoints_table(&self, cp: u64) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::delete(
                    checkpoints::table.filter(checkpoints::sequence_number.eq(cp as i64)),
                )
                .execute(conn)
                .map_err(IndexerError::from)
                .context("Failed to prune checkpoints table")?;

                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_epochs_table(&self, epoch: u64) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::delete(epochs::table.filter(epochs::epoch.eq(epoch as i64)))
                    .execute(conn)
                    .map_err(IndexerError::from)
                    .context("Failed to prune epochs table")?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_event_indices_table(&self, min_tx: u64, max_tx: u64) -> Result<(), IndexerError> {
        let (min_tx, max_tx) = (min_tx as i64, max_tx as i64);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                prune_tx_or_event_indice_table!(
                    event_emit_module,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_emit_module table"
                );
                prune_tx_or_event_indice_table!(
                    event_emit_package,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_emit_package table"
                );
                prune_tx_or_event_indice_table![
                    event_senders,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_senders table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_instantiation,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_instantiation table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_module,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_module table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_name,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_name table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_package,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_package table"
                ];
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_tx_indices_table(&self, min_tx: u64, max_tx: u64) -> Result<(), IndexerError> {
        let (min_tx, max_tx) = (min_tx as i64, max_tx as i64);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                prune_tx_or_event_indice_table!(
                    tx_senders,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_senders table"
                );
                prune_tx_or_event_indice_table!(
                    tx_recipients,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_recipients table"
                );
                prune_tx_or_event_indice_table![
                    tx_input_objects,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_input_objects table"
                ];
                prune_tx_or_event_indice_table![
                    tx_changed_objects,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_changed_objects table"
                ];
                prune_tx_or_event_indice_table![
                    tx_calls_pkg,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_calls_pkg table"
                ];
                prune_tx_or_event_indice_table![
                    tx_calls_mod,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_calls_mod table"
                ];
                prune_tx_or_event_indice_table![
                    tx_calls_fun,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_calls_fun table"
                ];
                prune_tx_or_event_indice_table![
                    tx_digests,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_digests table"
                ];
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_cp_tx_table(&self, cp: u64) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::delete(
                    pruner_cp_watermark::table
                        .filter(pruner_cp_watermark::checkpoint_sequence_number.eq(cp as i64)),
                )
                .execute(conn)
                .map_err(IndexerError::from)
                .context("Failed to prune pruner_cp_watermark table")?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn get_network_total_transactions_by_end_of_epoch(
        &self,
        epoch: u64,
    ) -> Result<u64, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::table
                .filter(checkpoints::epoch.eq(epoch as i64))
                .select(checkpoints::network_total_transactions)
                .order_by(checkpoints::sequence_number.desc())
                .first::<i64>(conn)
        })
        .context("Failed to get network total transactions in epoch")
        .map(|v| v as u64)
    }

    async fn execute_in_blocking_worker<F, R>(&self, f: F) -> Result<R, IndexerError>
    where
        F: FnOnce(Self) -> Result<R, IndexerError> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        let current_span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _guard = current_span.enter();
            f(this)
        })
        .await
        .map_err(Into::into)
        .and_then(std::convert::identity)
    }

    fn spawn_blocking_task<F, R>(
        &self,
        f: F,
    ) -> tokio::task::JoinHandle<std::result::Result<R, IndexerError>>
    where
        F: FnOnce(Self) -> Result<R, IndexerError> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        let current_span = tracing::Span::current();
        let guard = self.metrics.tokio_blocking_task_wait_latency.start_timer();
        tokio::task::spawn_blocking(move || {
            let _guard = current_span.enter();
            let _elapsed = guard.stop_and_record();
            f(this)
        })
    }

    fn spawn_task<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<Result<R, IndexerError>>
    where
        F: FnOnce(Self) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<R, IndexerError>> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        tokio::task::spawn(async move { f(this).await })
    }
}

#[async_trait]
impl<T: R2D2Connection> IndexerStore for PgIndexerStore<T> {
    async fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>, IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_latest_checkpoint_sequence_number())
            .await
    }

    async fn get_available_epoch_range(&self) -> Result<(u64, u64), IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_prunable_epoch_range())
            .await
    }

    async fn get_available_checkpoint_range(&self) -> Result<(u64, u64), IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_available_checkpoint_range())
            .await
    }

    async fn get_chain_identifier(&self) -> Result<Option<Vec<u8>>, IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_chain_identifier())
            .await
    }

    async fn get_latest_object_snapshot_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<u64>, IndexerError> {
        self.execute_in_blocking_worker(|this| {
            this.get_latest_object_snapshot_checkpoint_sequence_number()
        })
        .await
    }

    async fn persist_objects(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError> {
        if object_changes.is_empty() {
            return Ok(());
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects
            .start_timer();
        let objects = make_final_list_of_objects_to_commit(object_changes);
        let len = objects.len();

        let mut object_mutations = vec![];
        let mut object_deletions = vec![];
        for object in objects {
            match object {
                ObjectChangeToCommit::MutatedObject(mutation) => {
                    object_mutations.push(mutation);
                }
                ObjectChangeToCommit::DeletedObject(deletion) => {
                    object_deletions.push(deletion);
                }
            }
        }
        let mutation_len = object_mutations.len();
        let deletion_len = object_deletions.len();

        let object_mutation_chunks =
            chunk!(object_mutations, self.config.parallel_objects_chunk_size);
        let object_deletion_chunks =
            chunk!(object_deletions, self.config.parallel_objects_chunk_size);
        let mutation_futures = object_mutation_chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_object_mutation_chunk(c)))
            .collect::<Vec<_>>();
        futures::future::join_all(mutation_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!(
                    "Failed to join persist_object_mutation_chunk futures: {}",
                    e
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all object mutation chunks: {:?}",
                    e
                ))
            })?;
        let deletion_futures = object_deletion_chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_object_deletion_chunk(c)))
            .collect::<Vec<_>>();
        futures::future::join_all(deletion_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!(
                    "Failed to join persist_object_deletion_chunk futures: {}",
                    e
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all object deletion chunks: {:?}",
                    e
                ))
            })?;

        let elapsed = guard.stop_and_record();
        info!(
            elapsed,
            "Persisted {} objects with {} mutations and {} deletions ",
            len,
            mutation_len,
            deletion_len,
        );
        Ok(())
    }

    async fn backfill_objects_snapshot(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError> {
        if object_changes.is_empty() {
            return Ok(());
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_snapshot
            .start_timer();
        let objects = make_final_list_of_objects_to_commit(object_changes);
        let len = objects.len();
        let chunks = chunk!(objects, self.config.parallel_objects_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.backfill_objects_snapshot_chunk(c)))
            .collect::<Vec<_>>();

        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!(
                    "Failed to join backfill_objects_snapshot_chunk futures: {}",
                    e
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all objects snapshot chunks: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} objects snapshot", len);
        Ok(())
    }

    async fn persist_object_history(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError> {
        let skip_history = std::env::var("SKIP_OBJECT_HISTORY")
            .map(|val| val.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if skip_history {
            info!("skipping object history");
            return Ok(());
        }

        if object_changes.is_empty() {
            return Ok(());
        }
        let objects = make_objects_history_to_commit(object_changes);
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_history
            .start_timer();

        let len = objects.len();
        let chunks = chunk!(objects, self.config.parallel_objects_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_objects_history_chunk(c)))
            .collect::<Vec<_>>();

        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!(
                    "Failed to join persist_objects_history_chunk futures: {}",
                    e
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all objects history chunks: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} objects history", len);
        Ok(())
    }

    async fn update_objects_snapshot(
        &self,
        start_cp: u64,
        end_cp: u64,
    ) -> Result<(), IndexerError> {
        let skip_snapshot = std::env::var("SKIP_OBJECT_SNAPSHOT")
            .map(|val| val.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if skip_snapshot {
            info!("skipping object snapshot");
            return Ok(());
        }

        let guard = self.metrics.update_object_snapshot_latency.start_timer();

        self.spawn_blocking_task(move |this| this.update_objects_snapshot(start_cp, end_cp))
            .await
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to update objects snapshot: {:?}",
                    e
                ))
            })??;
        let elapsed = guard.stop_and_record();
        info!(
            elapsed,
            "Persisted snapshot for checkpoints from {} to {}", start_cp, end_cp
        );
        Ok(())
    }

    async fn persist_checkpoints(
        &self,
        checkpoints: Vec<IndexedCheckpoint>,
    ) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.persist_checkpoints(checkpoints))
            .await
    }

    async fn persist_transactions(
        &self,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_transactions
            .start_timer();
        let len = transactions.len();

        let chunks = chunk!(transactions, self.config.parallel_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_transactions_chunk(c)))
            .collect::<Vec<_>>();

        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to join persist_transactions_chunk futures: {}", e);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all transactions chunks: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} transactions", len);
        Ok(())
    }

    async fn persist_events(&self, events: Vec<IndexedEvent>) -> Result<(), IndexerError> {
        if events.is_empty() {
            return Ok(());
        }
        let len = events.len();
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_events
            .start_timer();
        let chunks = chunk!(events, self.config.parallel_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_events_chunk(c)))
            .collect::<Vec<_>>();

        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to join persist_events_chunk futures: {}", e);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all events chunks: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} events", len);
        Ok(())
    }

    async fn persist_displays(
        &self,
        display_updates: BTreeMap<String, StoredDisplay>,
    ) -> Result<(), IndexerError> {
        if display_updates.is_empty() {
            return Ok(());
        }

        self.spawn_blocking_task(move |this| this.persist_display_updates(display_updates))
            .await?
    }

    async fn persist_packages(&self, packages: Vec<IndexedPackage>) -> Result<(), IndexerError> {
        if packages.is_empty() {
            return Ok(());
        }
        self.execute_in_blocking_worker(move |this| this.persist_packages(packages))
            .await
    }

    async fn persist_event_indices(&self, indices: Vec<EventIndex>) -> Result<(), IndexerError> {
        if indices.is_empty() {
            return Ok(());
        }
        let len = indices.len();
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_event_indices
            .start_timer();
        let chunks = chunk!(indices, self.config.parallel_chunk_size);

        let futures = chunks
            .into_iter()
            .map(|chunk| {
                self.spawn_task(move |this: Self| async move {
                    this.persist_event_indices_chunk(chunk).await
                })
            })
            .collect::<Vec<_>>();
        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to join persist_event_indices_chunk futures: {}", e);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all event_indices chunks: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} event_indices chunks", len);
        Ok(())
    }

    async fn persist_tx_indices(&self, indices: Vec<TxIndex>) -> Result<(), IndexerError> {
        if indices.is_empty() {
            return Ok(());
        }
        let len = indices.len();
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_indices
            .start_timer();
        let chunks = chunk!(indices, self.config.parallel_chunk_size);

        let futures = chunks
            .into_iter()
            .map(|chunk| {
                self.spawn_task(move |this: Self| async move {
                    this.persist_tx_indices_chunk(chunk).await
                })
            })
            .collect::<Vec<_>>();
        futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to join persist_tx_indices_chunk futures: {}", e);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWriteError(format!(
                    "Failed to persist all tx_indices chunks: {:?}",
                    e
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} tx_indices chunks", len);
        Ok(())
    }

    async fn persist_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.persist_epoch(epoch))
            .await
    }

    async fn advance_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.advance_epoch(epoch))
            .await
    }

    async fn prune_epoch(&self, epoch: u64) -> Result<(), IndexerError> {
        let (mut min_cp, max_cp) = match self.get_checkpoint_range_for_epoch(epoch)? {
            (min_cp, Some(max_cp)) => Ok((min_cp, max_cp)),
            _ => Err(IndexerError::PostgresReadError(format!(
                "Failed to get checkpoint range for epoch {}",
                epoch
            ))),
        }?;

        // NOTE: for disaster recovery, min_cp is the min cp of the current epoch, which is likely
        // partially pruned already. min_prunable_cp is the min cp to be pruned.
        // By std::cmp::max, we will resume the pruning process from the next checkpoint, instead of
        // the first cp of the current epoch.
        let min_prunable_cp = self.get_min_prunable_checkpoint()?;
        min_cp = std::cmp::max(min_cp, min_prunable_cp);
        for cp in min_cp..=max_cp {
            // NOTE: the order of pruning tables is crucial:
            // 1. prune checkpoints table, checkpoints table is the source table of available range,
            // we prune it first to make sure that we always have full data for checkpoints within the available range;
            // 2. then prune tx_* tables;
            // 3. then prune pruner_cp_watermark table, which is the checkpoint pruning watermark table and also tx seq source
            // of a checkpoint to prune tx_* tables;
            // 4. lastly we prune epochs table when all checkpoints of the epoch have been pruned.
            info!(
                "Pruning checkpoint {} of epoch {} (min_prunable_cp: {})",
                cp, epoch, min_prunable_cp
            );
            self.execute_in_blocking_worker(move |this| this.prune_checkpoints_table(cp))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to prune checkpoint {}: {}", cp, e);
                });

            let (min_tx, max_tx) = self.get_transaction_range_for_checkpoint(cp)?;
            self.execute_in_blocking_worker(move |this| {
                this.prune_tx_indices_table(min_tx, max_tx)
            })
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to prune transactions for cp {}: {}", cp, e);
            });
            info!(
                "Pruned transactions for checkpoint {} from tx {} to tx {}",
                cp, min_tx, max_tx
            );
            self.execute_in_blocking_worker(move |this| {
                this.prune_event_indices_table(min_tx, max_tx)
            })
            .await
            .unwrap_or_else(|e| {
                tracing::error!(
                    "Failed to prune events of transactions for cp {}: {}",
                    cp,
                    e
                );
            });
            info!(
                "Pruned events of transactions for checkpoint {} from tx {} to tx {}",
                cp, min_tx, max_tx
            );
            self.metrics.last_pruned_transaction.set(max_tx as i64);

            self.execute_in_blocking_worker(move |this| this.prune_cp_tx_table(cp))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!(
                        "Failed to prune pruner_cp_watermark table for cp {}: {}",
                        cp,
                        e
                    );
                });
            info!("Pruned checkpoint {} of epoch {}", cp, epoch);
            self.metrics.last_pruned_checkpoint.set(cp as i64);
        }

        // NOTE: prune epochs table last, otherwise get_checkpoint_range_for_epoch would fail.
        self.execute_in_blocking_worker(move |this| this.prune_epochs_table(epoch))
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to prune epoch table for epoch {}: {}", epoch, e);
            });
        Ok(())
    }

    async fn get_network_total_transactions_by_end_of_epoch(
        &self,
        epoch: u64,
    ) -> Result<u64, IndexerError> {
        self.execute_in_blocking_worker(move |this| {
            this.get_network_total_transactions_by_end_of_epoch(epoch)
        })
        .await
    }

    fn as_any(&self) -> &dyn StdAny {
        self
    }

    /// Persist protocol configs and feature flags until the protocol version for the latest epoch
    /// we have stored in the db, inclusive.
    fn persist_protocol_configs_and_feature_flags(
        &self,
        chain_id: Vec<u8>,
    ) -> Result<(), IndexerError> {
        let chain_id = ChainIdentifier::from(
            CheckpointDigest::try_from(chain_id).expect("Unable to convert chain id"),
        );

        let mut all_configs = vec![];
        let mut all_flags = vec![];

        let (start_version, end_version) = self.get_protocol_version_index_range()?;
        info!(
            "Persisting protocol configs with start_version: {}, end_version: {}",
            start_version, end_version
        );

        // Gather all protocol configs and feature flags for all versions between start and end.
        for version in start_version..=end_version {
            let protocol_configs = ProtocolConfig::get_for_version_if_supported(
                (version as u64).into(),
                chain_id.chain(),
            )
            .ok_or(IndexerError::GenericError(format!(
                "Unable to fetch protocol version {} and chain {:?}",
                version,
                chain_id.chain()
            )))?;
            let configs_vec = protocol_configs
                .attr_map()
                .into_iter()
                .map(|(k, v)| StoredProtocolConfig {
                    protocol_version: version,
                    config_name: k,
                    config_value: v.map(|v| v.to_string()),
                })
                .collect::<Vec<_>>();
            all_configs.extend(configs_vec);

            let feature_flags = protocol_configs
                .feature_map()
                .into_iter()
                .map(|(k, v)| StoredFeatureFlag {
                    protocol_version: version,
                    flag_name: k,
                    flag_value: v,
                })
                .collect::<Vec<_>>();
            all_flags.extend(feature_flags);
        }

        // Now insert all of them into the db.
        // TODO: right now the size of these updates is manageable but later we may consider batching.
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                insert_or_ignore_into!(protocol_configs::table, all_configs.clone(), conn);
                insert_or_ignore_into!(feature_flags::table, all_flags.clone(), conn);
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )?;
        Ok(())
    }
}

/// Construct deleted objects and mutated objects to commit.
/// In particular, filter mutated objects updates that would
/// be override immediately.
fn make_final_list_of_objects_to_commit(
    tx_object_changes: Vec<TransactionObjectChangesToCommit>,
) -> Vec<ObjectChangeToCommit> {
    let deleted_objects = tx_object_changes
        .clone()
        .into_iter()
        .flat_map(|changes| changes.deleted_objects)
        .map(|o| (o.object_id, o.into()))
        .collect::<HashMap<ObjectID, StoredDeletedObject>>();

    let mutated_objects = tx_object_changes
        .into_iter()
        .flat_map(|changes| changes.changed_objects);
    let mut latest_objects = HashMap::new();
    for object in mutated_objects {
        if deleted_objects.contains_key(&object.object.id()) {
            continue;
        }
        match latest_objects.entry(object.object.id()) {
            Entry::Vacant(e) => {
                e.insert(object);
            }
            Entry::Occupied(mut e) => {
                if object.object.version() > e.get().object.version() {
                    e.insert(object);
                }
            }
        }
    }
    deleted_objects
        .into_values()
        .map(ObjectChangeToCommit::DeletedObject)
        .chain(
            latest_objects
                .into_values()
                .map(StoredObject::from)
                .map(ObjectChangeToCommit::MutatedObject),
        )
        .collect()
}

fn make_objects_history_to_commit(
    tx_object_changes: Vec<TransactionObjectChangesToCommit>,
) -> Vec<ObjectChangeToCommit> {
    let deleted_objects: Vec<StoredDeletedObject> = tx_object_changes
        .clone()
        .into_iter()
        .flat_map(|changes| changes.deleted_objects)
        .map(|o| o.into())
        .collect();
    let mutated_objects: Vec<StoredObject> = tx_object_changes
        .into_iter()
        .flat_map(|changes| changes.changed_objects)
        .map(|o| o.into())
        .collect();
    deleted_objects
        .into_iter()
        .map(ObjectChangeToCommit::DeletedObject)
        .chain(
            mutated_objects
                .into_iter()
                .map(ObjectChangeToCommit::MutatedObject),
        )
        .collect()
}
