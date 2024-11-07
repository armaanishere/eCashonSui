// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use futures::future::join_all;
use mysten_metrics::spawn_monitored_task;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use strum::IntoEnumIterator;
use strum_macros;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::config::RetentionConfig;
use crate::errors::IndexerError;
use crate::handlers::pruners::spawn_pruners;
use crate::store::PgIndexerStore;
use crate::{metrics::IndexerMetrics, store::IndexerStore, types::IndexerResult};

const MAX_DELAY_MS: u64 = 10000;

pub struct Pruner {
    pub store: PgIndexerStore,
    pub retention_policies: HashMap<PrunableTable, u64>,
    pub metrics: IndexerMetrics,
}

/// Enum representing tables that the pruner is allowed to prune. This corresponds to table names in
/// the database, and should be used in lieu of string literals. This enum is also meant to
/// facilitate the process of determining which unit (epoch, cp, or tx) should be used for the
/// table's range. Pruner will ignore any table that is not listed here.
#[derive(
    Debug,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::AsRefStr,
    Hash,
    Serialize,
    Deserialize,
    Clone,
    Copy,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PrunableTable {
    ObjectsHistory,
    Transactions,
    Events,

    EventEmitPackage,
    EventEmitModule,
    EventSenders,
    EventStructInstantiation,
    EventStructModule,
    EventStructName,
    EventStructPackage,

    TxAffectedAddresses,
    TxAffectedObjects,
    TxCallsPkg,
    TxCallsMod,
    TxCallsFun,
    TxChangedObjects,
    TxDigests,
    TxInputObjects,
    TxKinds,

    Checkpoints,
}

impl PrunableTable {
    pub fn select_reader_lo(&self, cp: u64, tx: u64) -> u64 {
        match self {
            PrunableTable::ObjectsHistory => cp,
            PrunableTable::Transactions => tx,
            PrunableTable::Events => tx,

            PrunableTable::EventEmitPackage => tx,
            PrunableTable::EventEmitModule => tx,
            PrunableTable::EventSenders => tx,
            PrunableTable::EventStructInstantiation => tx,
            PrunableTable::EventStructModule => tx,
            PrunableTable::EventStructName => tx,
            PrunableTable::EventStructPackage => tx,

            PrunableTable::TxAffectedAddresses => tx,
            PrunableTable::TxAffectedObjects => tx,
            PrunableTable::TxCallsPkg => tx,
            PrunableTable::TxCallsMod => tx,
            PrunableTable::TxCallsFun => tx,
            PrunableTable::TxChangedObjects => tx,
            PrunableTable::TxDigests => tx,
            PrunableTable::TxInputObjects => tx,
            PrunableTable::TxKinds => tx,

            PrunableTable::Checkpoints => cp,
        }
    }
}

impl Pruner {
    /// Instantiates a pruner with default retention and overrides. Pruner will finalize the
    /// retention policies so there is a value for every prunable table.
    pub fn new(
        store: PgIndexerStore,
        retention_config: RetentionConfig,
        metrics: IndexerMetrics,
    ) -> Result<Self, IndexerError> {
        let retention_policies = retention_config.retention_policies();

        Ok(Self {
            store,
            retention_policies,
            metrics,
        })
    }

    pub async fn start(&self, cancel: CancellationToken) -> IndexerResult<()> {
        let store_clone = self.store.clone();
        let retention_policies = self.retention_policies.clone();
        let cancel_clone = cancel.clone();
        spawn_monitored_task!(update_watermarks_lower_bounds_task(
            store_clone,
            retention_policies,
            cancel_clone
        ));

        let mut table_tasks = spawn_pruners(cancel.clone(), self.store.clone());

        for table in PrunableTable::iter() {
            let store_clone = self.store.clone();
            let cancel_clone = cancel.clone();

            table_tasks.push(spawn_monitored_task!(update_pruner_watermark_task(
                store_clone,
                table,
                cancel_clone
            )));
        }

        cancel.cancelled().await;

        join_all(table_tasks).await;

        Ok(())
    }
}

/// Task to periodically query the `watermarks` table and update the lower bounds for all watermarks
/// if the entry exceeds epoch-level retention policy.
async fn update_watermarks_lower_bounds_task(
    store: PgIndexerStore,
    retention_policies: HashMap<PrunableTable, u64>,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Pruner watermark lower bound update task cancelled.");
                return Ok(());
            }
            _ = interval.tick() => {
                if let Err(err) = update_watermarks_lower_bounds(&store, &retention_policies, &cancel).await {
                    error!("Failed to update watermarks lower bounds: {}", err);
                }
            }
        }
    }
}

/// Fetches all entries from the `watermarks` table, and updates the `reader_lo` for each entry if
/// its epoch range exceeds the respective retention policy.
async fn update_watermarks_lower_bounds(
    store: &PgIndexerStore,
    retention_policies: &HashMap<PrunableTable, u64>,
    cancel: &CancellationToken,
) -> IndexerResult<()> {
    let (watermarks, _) = store.get_watermarks().await?;
    let mut lower_bound_updates = vec![];

    for watermark in watermarks.iter() {
        if cancel.is_cancelled() {
            info!("Reader watermark lower bound update task cancelled.");
            return Ok(());
        }

        let Some(prunable_table) = watermark.entity() else {
            continue;
        };

        let Some(epochs_to_keep) = retention_policies.get(&prunable_table) else {
            error!(
                "No retention policy found for prunable table {}",
                prunable_table
            );
            continue;
        };

        if let Some(new_epoch_lo) = watermark.new_epoch_lo(*epochs_to_keep) {
            lower_bound_updates.push((prunable_table, new_epoch_lo));
        };
    }

    if !lower_bound_updates.is_empty() {
        store
            .update_watermarks_lower_bound(lower_bound_updates)
            .await?;
        info!("Finished updating lower bounds for watermarks");
    }

    Ok(())
}

/// Task to periodically update `pruner_hi` to the local `reader_lo` if it sees a newer
/// value for `reader_lo`.
async fn update_pruner_watermark_task(
    store: PgIndexerStore,
    table: PrunableTable,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let (watermark, _) = store.get_watermark(table).await?;
    let mut local_reader_lo = watermark.pruner_upper_bound().unwrap();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Pruner watermark lower bound update task cancelled.");
                return Ok(());
            }
            _ = interval.tick() => {
                let (watermark, latest_db_timestamp) = store.get_watermark(table).await?;
                let reader_lo_timestamp = watermark.timestamp_ms;
                let new_reader_lo = watermark.pruner_upper_bound().unwrap();
                let should_update = new_reader_lo > local_reader_lo || local_reader_lo > watermark.pruner_hi as u64;
                let update_value = if new_reader_lo > local_reader_lo { new_reader_lo } else { local_reader_lo };

                if should_update {
                    let delay_duration = MAX_DELAY_MS.saturating_sub((latest_db_timestamp - reader_lo_timestamp) as u64);

                    if delay_duration > 0 {
                        tokio::time::sleep(Duration::from_millis(delay_duration)).await;
                    }

                    if let Err(err) = store.update_pruner_watermark(table, update_value).await {
                        error!("Failed to update pruner watermark: {}", err);
                    }

                    local_reader_lo = update_value;
                }
            }
        }
    }
}
