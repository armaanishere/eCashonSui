// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::ingestion::remote_client::RemoteIngestionClient;
use crate::ingestion::Error as IngestionError;
use crate::ingestion::Result as IngestionResult;
use crate::metrics::IndexerMetrics;
use backoff::Error as BE;
use backoff::ExponentialBackoff;
use std::sync::Arc;
use std::time::Duration;
use sui_storage::blob::Blob;
use sui_types::full_checkpoint_content::CheckpointData;
use sui_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio_util::bytes::Bytes;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use url::Url;

/// Wait at most this long between retries for transient errors.
const MAX_TRANSIENT_RETRY_INTERVAL: Duration = Duration::from_secs(60);

#[async_trait::async_trait]
pub(crate) trait IngestionClientTrait: Send + Sync {
    async fn fetch(
        &self,
        checkpoint: CheckpointSequenceNumber,
    ) -> Result<Bytes, BE<IngestionError>>;
}

#[derive(Clone)]
pub(crate) struct IngestionClient {
    client: Arc<dyn IngestionClientTrait>,
    metrics: Arc<IndexerMetrics>,
}

impl IngestionClient {
    pub(crate) fn new_remote(url: Url, metrics: Arc<IndexerMetrics>) -> IngestionResult<Self> {
        let client = Arc::new(RemoteIngestionClient::new(url, metrics.clone())?);
        Ok(IngestionClient { client, metrics })
    }

    pub(crate) async fn fetch(
        &self,
        checkpoint: CheckpointSequenceNumber,
        cancel: &CancellationToken,
    ) -> IngestionResult<Arc<CheckpointData>> {
        let client = self.client.clone();
        let request = move || {
            let client = client.clone();
            async move {
                if cancel.is_cancelled() {
                    return Err(BE::permanent(IngestionError::Cancelled));
                }

                let bytes = client.fetch(checkpoint).await?;

                self.metrics.total_ingested_bytes.inc_by(bytes.len() as u64);
                let data: CheckpointData = Blob::from_bytes(&bytes).map_err(|e| {
                    self.metrics.inc_retry(
                        checkpoint,
                        "deserialization",
                        IngestionError::DeserializationError(checkpoint, e),
                    )
                })?;

                Ok(data)
            }
        };

        // Keep backing off until we are waiting for the max interval, but don't give up.
        let backoff = ExponentialBackoff {
            max_interval: MAX_TRANSIENT_RETRY_INTERVAL,
            max_elapsed_time: None,
            ..Default::default()
        };

        let guard = self.metrics.ingested_checkpoint_latency.start_timer();
        let data = backoff::future::retry(backoff, request).await?;
        let elapsed = guard.stop_and_record();

        debug!(
            checkpoint,
            elapsed_ms = elapsed * 1000.0,
            "Fetched checkpoint"
        );

        self.metrics.total_ingested_checkpoints.inc();

        self.metrics
            .total_ingested_transactions
            .inc_by(data.transactions.len() as u64);

        self.metrics.total_ingested_events.inc_by(
            data.transactions
                .iter()
                .map(|tx| tx.events.as_ref().map_or(0, |evs| evs.data.len()) as u64)
                .sum(),
        );

        self.metrics.total_ingested_inputs.inc_by(
            data.transactions
                .iter()
                .map(|tx| tx.input_objects.len() as u64)
                .sum(),
        );

        self.metrics.total_ingested_outputs.inc_by(
            data.transactions
                .iter()
                .map(|tx| tx.output_objects.len() as u64)
                .sum(),
        );

        Ok(Arc::new(data))
    }
}
