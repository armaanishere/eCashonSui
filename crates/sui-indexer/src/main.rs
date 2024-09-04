// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use sui_indexer::config::Command;
use sui_indexer::database::ConnectionPool;
use sui_indexer::db::{
    check_db_migration_consistency, get_pool_connection, new_connection_pool, reset_database,
    run_migrations,
};
use sui_indexer::indexer::Indexer;
use sui_indexer::store::PgIndexerStore;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use sui_indexer::metrics::{
    spawn_connection_pool_metric_collector, start_prometheus_server, IndexerMetrics,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = sui_indexer::config::IndexerConfig::parse();

    // NOTE: this is to print out tracing like info, warn & error.
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    warn!("WARNING: Sui indexer is still experimental and we expect occasional breaking changes that require backfills.");

    let (_registry_service, registry) = start_prometheus_server(opts.metrics_address)?;
    mysten_metrics::init_metrics(&registry);
    let indexer_metrics = IndexerMetrics::new(&registry);

    let connection_pool =
        new_connection_pool(opts.database_url.as_str(), &opts.connection_pool_config)?;
    let pool = ConnectionPool::new(
        opts.database_url.clone(),
        opts.connection_pool_config.clone(),
    )
    .await?;
    spawn_connection_pool_metric_collector(indexer_metrics.clone(), connection_pool.clone());

    match opts.command {
        Command::Indexer {
            ingestion_config,
            snapshot_config,
            pruning_options,
            restore_config,
        } => {
            // Make sure to run all migrations on startup, and also serve as a compatibility check.
            run_migrations(&mut get_pool_connection(&connection_pool)?).await?;

            let store = PgIndexerStore::new(
                connection_pool,
                pool,
                restore_config,
                indexer_metrics.clone(),
            );

            Indexer::start_writer_with_config(
                &ingestion_config,
                store,
                indexer_metrics,
                snapshot_config,
                pruning_options,
                CancellationToken::new(),
            )
            .await?;
        }
        Command::JsonRpcService(json_rpc_config) => {
            check_db_migration_consistency(&mut get_pool_connection(&connection_pool)?)?;

            Indexer::start_reader(&json_rpc_config, &registry, connection_pool, pool).await?;
        }
        Command::ResetDatabase { force } => {
            if !force {
                return Err(anyhow::anyhow!(
                    "Resetting the DB requires use of the `--force` flag",
                ));
            }

            reset_database(&mut get_pool_connection(&connection_pool)?).await?;
        }
        Command::RunMigrations => {
            run_migrations(&mut get_pool_connection(&connection_pool)?).await?;
        }
    }

    Ok(())
}
