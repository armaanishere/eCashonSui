// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use async_trait::async_trait;
use std::cmp::min;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;
use tap::TapFallible;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender;
use tracing::info;

use mysten_metrics::spawn_monitored_task;
use sui_data_ingestion_core::{
    DataIngestionMetrics, IndexerExecutor, ProgressStore, ReaderOptions, Worker, WorkerPool,
};
use sui_types::base_types::{ObjectID, TransactionDigest};
use sui_types::full_checkpoint_content::{CheckpointData, CheckpointTransaction};
use sui_types::messages_checkpoint::CheckpointSequenceNumber;
use sui_types::transaction::TransactionDataAPI;
use sui_types::transaction::TransactionKind;

use crate::sui_checkpoint_ingestion::{Task, Tasks};

pub struct IndexerBuilder<D, F, M> {
    datasource: D,
    filter: F,
    data_mapper: M,
    back_fill_strategy: BackfillStrategy,
}

impl<D, F, M> IndexerBuilder<D, F, M> {
    pub fn new(datasource: D, filter: F, data_mapper: M) -> IndexerBuilder<D, F, M> {
        IndexerBuilder {
            datasource,
            filter,
            data_mapper,
            back_fill_strategy: BackfillStrategy::Simple,
        }
    }
    pub fn build<R, P>(self, persistent: P) -> Indexer<P, D, F, M>
    where
        P: Persistent<R>,
    {
        Indexer {
            name: "".to_string(),
            storage: persistent,
            datasource: self.datasource.into(),
            backfill_strategy: self.back_fill_strategy,
            start_from_checkpoint: 0,
            filter: self.filter,
            data_mapper: self.data_mapper,
            genesis_checkpoint: 0,
        }
    }

    pub fn with_transaction_filter<NewFilter>(
        self,
        filter: NewFilter,
    ) -> IndexerBuilder<D, NewFilter, M> {
        IndexerBuilder {
            datasource: self.datasource,
            filter,
            data_mapper: self.data_mapper,
            back_fill_strategy: self.back_fill_strategy,
        }
    }

    pub fn with_back_fill_strategy(mut self, back_fill: BackfillStrategy) -> Self {
        self.back_fill_strategy = back_fill;
        self
    }

    pub fn with_data_processor<NewMapper>(
        self,
        data_processor: NewMapper,
    ) -> IndexerBuilder<D, F, NewMapper> {
        IndexerBuilder {
            datasource: self.datasource,
            filter: self.filter,
            data_mapper: data_processor,
            back_fill_strategy: self.back_fill_strategy,
        }
    }
}

pub struct Indexer<P, D, F, M> {
    name: String,
    storage: P,
    datasource: Arc<D>,
    filter: F,
    data_mapper: M,
    backfill_strategy: BackfillStrategy,
    start_from_checkpoint: u64,
    genesis_checkpoint: u64,
}

impl<P, D, F, M> Indexer<P, D, F, M> {
    pub async fn start<T, R>(mut self) -> Result<(), Error>
    where
        D: Datasource<T, P, R> + 'static + Send + Sync,
        F: DataFilter<T> + 'static + Clone,
        M: DataMapper<T, R> + 'static + Clone,
        P: Persistent<R> + 'static,
    {
        // Update tasks first
        let tasks = self.storage.tasks()?;
        // create checkpoint workers base on backfill config and existing tasks in the db
        match tasks.live_task() {
            None => {
                // Scenario 1: No task in database, start live task and backfill tasks
                // if resume_from_checkpoint, use it for the latest task, if not set, use bridge_genesis_checkpoint
                self.storage.register_task(
                    format!("{} - Live", self.name),
                    self.start_from_checkpoint,
                    i64::MAX,
                )?;
                // Create backfill tasks
                if self.start_from_checkpoint != self.genesis_checkpoint {
                    self.create_backfill_tasks(self.genesis_checkpoint)?
                }
            }
            Some(mut live_task) => {
                if self.start_from_checkpoint > live_task.checkpoint {
                    // Scenario 2: there are existing tasks in DB and start_from_checkpoint > current checkpoint
                    // create backfill task to finish at start_from_checkpoint
                    // update live task to start from start_from_checkpoint and finish at u64::MAX
                    self.create_backfill_tasks(live_task.checkpoint)?;
                    live_task.checkpoint = self.start_from_checkpoint;
                    self.storage.update_task(live_task)?;
                } else {
                    // Scenario 3: start_from_checkpoint < current checkpoint
                    // ignore start_from_checkpoint, resume all task as it is.
                }
            }
        }

        // get updated tasks from storage and start workers
        let updated_tasks = self.storage.tasks()?;
        // Start latest checkpoint worker
        // Tasks are ordered in checkpoint descending order, realtime update task always come first
        // tasks won't be empty here, ok to unwrap.
        let (live_task, backfill_tasks) = updated_tasks.split_first().unwrap();

        let live_task_future = self.datasource.start_ingestion_task(
            live_task.task_name.clone(),
            live_task.target_checkpoint,
            self.storage.clone(),
            self.filter.clone(),
            self.data_mapper.clone(),
        );

        let backfill_tasks = backfill_tasks.to_vec();
        let storage_clone = self.storage.clone();
        let filter_clone = self.filter.clone();
        let data_mapper_clone = self.data_mapper.clone();
        let datasource_clone = self.datasource.clone();

        let handle = spawn_monitored_task!(async {
            // Execute task one by one
            for backfill_task in backfill_tasks {
                datasource_clone
                    .start_ingestion_task(
                        backfill_task.task_name.clone(),
                        backfill_task.target_checkpoint,
                        storage_clone.clone(),
                        filter_clone.clone(),
                        data_mapper_clone.clone(),
                    )
                    .await
                    .expect("Backfill task failed");
            }
        });
        live_task_future.await?;
        tokio::try_join!(handle)?;

        Ok(())
    }
    // Create backfill tasks according to backfill strategy
    fn create_backfill_tasks<R>(&mut self, mut current_cp: u64) -> Result<(), Error>
    where
        P: Persistent<R> + 'static,
    {
        return match self.backfill_strategy {
            BackfillStrategy::Simple => self.storage.register_task(
                format!("{} - backfill - {}", self.name, TransactionDigest::random()),
                current_cp,
                self.start_from_checkpoint as i64,
            ),
            BackfillStrategy::Partitioned { task_size } => {
                while current_cp < self.start_from_checkpoint {
                    let target_cp = min(current_cp + task_size, self.start_from_checkpoint);
                    self.storage.register_task(
                        format!("{} - backfill - {}", self.name, TransactionDigest::random()),
                        current_cp,
                        target_cp as i64,
                    )?;
                    current_cp = target_cp;
                }
                Ok(())
            }
            BackfillStrategy::Disabled => Ok(()),
        };
    }
}

pub trait Persistent<T>: IndexerProgressStore + Sync + Send + Clone {
    fn write(&self, data: Vec<T>) -> Result<(), Error>;
}

#[async_trait]
pub trait IndexerProgressStore: Send {
    async fn load(&self, task_name: String) -> anyhow::Result<CheckpointSequenceNumber>;
    async fn save(
        &mut self,
        task_name: String,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> anyhow::Result<()>;

    fn tasks(&self) -> Result<Vec<Task>, anyhow::Error>;

    fn register_task(
        &mut self,
        task_name: String,
        checkpoint: u64,
        target_checkpoint: i64,
    ) -> Result<(), anyhow::Error>;

    fn update_task(&mut self, task: Task) -> Result<(), anyhow::Error>;
}

#[async_trait]
pub trait Datasource<T, P, R> {
    async fn start_ingestion_task<F, M>(
        &self,
        task_name: String,
        target_checkpoint: u64,
        storage: P,
        filter: F,
        data_mapper: M,
    ) -> Result<(), anyhow::Error>
    where
        F: DataFilter<T> + 'static,
        M: DataMapper<T, R> + 'static;
}

pub struct SuiCheckpointDatasource {
    remote_store_url: String,
    concurrency: usize,
    checkpoint_path: PathBuf,
    metrics: DataIngestionMetrics,
}
impl SuiCheckpointDatasource {
    pub fn new(
        remote_store_url: String,
        concurrency: usize,
        checkpoint_path: PathBuf,
        metrics: DataIngestionMetrics,
    ) -> Self {
        SuiCheckpointDatasource {
            remote_store_url,
            concurrency,
            checkpoint_path: checkpoint_path.into(),
            metrics,
        }
    }
}

#[async_trait]
impl<P: Persistent<R> + 'static, R: Sync + Send + 'static> Datasource<CheckpointTxnData, P, R>
    for SuiCheckpointDatasource
{
    async fn start_ingestion_task<F, M>(
        &self,
        task_name: String,
        target_checkpoint: u64,
        storage: P,
        filter: F,
        data_mapper: M,
    ) -> Result<(), anyhow::Error>
    where
        F: DataFilter<CheckpointTxnData> + 'static,
        M: DataMapper<CheckpointTxnData, R> + 'static,
    {
        let (exit_sender, exit_receiver) = oneshot::channel();
        let progress_store = ProgressStoreWrapper {
            store: storage.clone(),
            exit_checkpoint: target_checkpoint,
            exit_sender: Some(exit_sender),
        };

        let mut executor = IndexerExecutor::new(progress_store, 1, self.metrics.clone());
        let worker = IndexerWorker::new(storage, filter, data_mapper);
        let worker_pool = WorkerPool::new(worker, task_name, self.concurrency);
        executor.register(worker_pool).await?;
        executor
            .run(
                self.checkpoint_path.clone(),
                Some(self.remote_store_url.clone()),
                vec![], // optional remote store access options
                ReaderOptions::default(),
                exit_receiver,
            )
            .await?;
        Ok(())
    }
}

pub enum BackfillStrategy {
    Simple,
    Partitioned { task_size: u64 },
    Disabled,
}

pub trait DataFilter<T>: Sync + Send {
    fn filter(&self, data: &T) -> bool;
}

#[derive(Clone)]
pub struct SuiInputObjectFilter {
    pub object_id: ObjectID,
}

impl DataFilter<CheckpointTxnData> for SuiInputObjectFilter {
    fn filter(&self, (tx, _, _): &CheckpointTxnData) -> bool {
        let txn_data = tx.transaction.transaction_data();
        if let TransactionKind::ProgrammableTransaction(_) = txn_data.kind() {
            return tx
                .input_objects
                .iter()
                .any(|obj| obj.id() == self.object_id);
        };
        false
    }
}

pub trait DataMapper<T, R>: Sync + Send {
    fn map(&self, data: T) -> Result<Vec<R>, anyhow::Error>;
}

pub struct ProgressStoreWrapper<P> {
    pub store: P,
    pub exit_checkpoint: u64,
    pub exit_sender: Option<Sender<()>>,
}

#[async_trait]
impl<P: IndexerProgressStore> ProgressStore for ProgressStoreWrapper<P> {
    async fn load(&mut self, task_name: String) -> Result<CheckpointSequenceNumber, anyhow::Error> {
        self.store.load(task_name).await
    }

    async fn save(
        &mut self,
        task_name: String,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> anyhow::Result<()> {
        if checkpoint_number >= self.exit_checkpoint {
            if let Some(sender) = self.exit_sender.take() {
                let _ = sender.send(());
            }
        }
        self.store.save(task_name, checkpoint_number).await
    }
}

pub struct IndexerWorker<F, M, P, T, R> {
    filter: F,
    data_mapper: M,
    persistent: P,
    phantom_data: PhantomData<T>,
    phantom_data2: PhantomData<R>,
}

impl<F, M, P, T, R> IndexerWorker<F, M, P, T, R> {
    pub fn new(persistent: P, filter: F, data_mapper: M) -> Self {
        Self {
            filter,
            persistent,
            data_mapper,
            phantom_data: Default::default(),
            phantom_data2: Default::default(),
        }
    }
}

pub type CheckpointTxnData = (CheckpointTransaction, u64, u64);

#[async_trait]
impl<F, M, P, R: Sync + Send> Worker for IndexerWorker<F, M, P, CheckpointTxnData, R>
where
    F: DataFilter<CheckpointTxnData>,
    M: DataMapper<CheckpointTxnData, R>,
    P: Persistent<R>,
{
    async fn process_checkpoint(&self, checkpoint: CheckpointData) -> anyhow::Result<()> {
        info!(
            "Processing checkpoint [{}] {}: {}",
            checkpoint.checkpoint_summary.epoch,
            checkpoint.checkpoint_summary.sequence_number,
            checkpoint.transactions.len(),
        );
        let checkpoint_num = checkpoint.checkpoint_summary.sequence_number;
        let timestamp_ms = checkpoint.checkpoint_summary.timestamp_ms;

        let bridge_data = checkpoint
            .transactions
            .into_iter()
            .filter(|data| {
                self.filter
                    .filter(&(data.clone(), checkpoint_num, timestamp_ms))
            })
            .try_fold(vec![], |mut result, txn| {
                result.append(&mut self.data_mapper.map((txn, checkpoint_num, timestamp_ms))?);
                Ok::<_, anyhow::Error>(result)
            })?;

        self.persistent.write(bridge_data).tap_ok(|_| {
            info!("Processed checkpoint [{}] successfully", checkpoint_num,);
            // TODO
            /*            self.metrics
            .last_committed_sui_checkpoint
            .set(checkpoint_num as i64);*/
        })
    }
}
