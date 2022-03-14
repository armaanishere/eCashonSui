// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
extern crate core;

use std::path::PathBuf;
use structopt::StructOpt;
use sui::config::{Config, NetworkConfig};
use sui::sui_commands::SuiCommand;

use opentelemetry::global;
use opentelemetry::sdk::propagation::TraceContextPropagator;
use tracing::info;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

#[cfg(test)]
#[path = "unit_tests/cli_tests.rs"]
mod cli_tests;

#[derive(StructOpt)]
#[structopt(
    name = "Sui Local",
    about = "A Byzantine fault tolerant chain with low-latency finality and high throughput",
    rename_all = "kebab-case"
)]
struct SuiOpt {
    #[structopt(subcommand)]
    command: SuiCommand,
    #[structopt(long, default_value = "./network.conf")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // TODO: reorganize different telemetry options so they can use the same registry
    // Code to add logging/tracing config from environment, including RUST_LOG
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // See [[dev-docs/observability.md]] for more information on span logging.
    if std::env::var("SUI_JSON_SPAN_LOGS").is_ok() {
        // See https://www.lpalmieri.com/posts/2020-09-27-zero-to-production-4-are-we-observable-yet/#5-7-tracing-bunyan-formatter
        // Also Bunyan layer addes JSON logging for tracing spans with duration information
        let formatting_layer = BunyanFormattingLayer::new(
            "sui".into(),
            // Output the formatted spans to stdout.
            std::io::stdout,
        );
        // The `with` method is provided by `SubscriberExt`, an extension
        // trait for `Subscriber` exposed by `tracing_subscriber`
        let subscriber = Registry::default()
            .with(env_filter)
            .with(JsonStorageLayer)
            .with(formatting_layer);
        // `set_global_default` can be used by applications to specify
        // what subscriber should be used to process spans.
        set_global_default(subscriber).expect("Failed to set subscriber");

        info!("Enabling JSON and span logging");
    } else if std::env::var("SUI_TOKIO_CONSOLE").is_ok() {
        console_subscriber::init();
    } else {
        // Standard env filter (RUST_LOG) with standard formatter
        let subscriber = Registry::default()
            .with(env_filter)
            .with(fmt::layer().with_ansi(true).with_writer(std::io::stdout));

        // We assume you would not enable both SUI_JSON_SPAN_LOGS and open telemetry at same time, but who knows?
        if std::env::var("SUI_TRACING_ENABLE").is_ok() {
            // Install a tracer to send traces to Jaeger.  Batching for better performance.
            let tracer = opentelemetry_jaeger::new_pipeline()
                .with_service_name("sui")
                .with_max_packet_size(9216) // Default max UDP packet size on OSX
                .with_auto_split_batch(true) // Auto split batches so they fit under packet size
                .install_batch(opentelemetry::runtime::Tokio)
                .expect("Could not create async Tracer");

            // Create a tracing subscriber with the configured tracer
            let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

            // Enable Trace Contexts for tying spans together
            global::set_text_map_propagator(TraceContextPropagator::new());

            set_global_default(subscriber.with(telemetry)).expect("Failed to set subscriber");
        } else {
            set_global_default(subscriber).expect("Failed to set subscriber");
        }
    }

    let options: SuiOpt = SuiOpt::from_args();
    let network_conf_path = options.config;
    let mut config = NetworkConfig::read_or_create(&network_conf_path)?;

    options.command.execute(&mut config).await
}
