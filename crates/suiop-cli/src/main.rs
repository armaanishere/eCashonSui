// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Parser;
use suioplib::{
    cli::{
        ci_cmd, docker_cmd, iam_cmd, incidents_cmd, load_environment_cmd, pulumi_cmd, service_cmd,
        CIArgs, DockerArgs, IAMArgs, IncidentsArgs, LoadEnvironmentArgs, PulumiArgs, ServiceArgs,
    },
    DEBUG_MODE,
};
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    FmtSubscriber,
};

#[derive(Parser, Debug)]
#[command(author="build@mystenlabs.com", version, about, long_about = None)]
pub(crate) struct SuiOpArgs {
    /// The resource type we're operating on.
    #[command(subcommand)]
    resource: Resource,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum Resource {
    #[clap(aliases = ["d"])]
    Docker(DockerArgs),
    #[clap()]
    Iam(IAMArgs),
    #[clap(aliases = ["inc", "i"])]
    Incidents(IncidentsArgs),
    #[clap(aliases = ["p"])]
    Pulumi(PulumiArgs),
    #[clap(aliases = ["s", "svc"])]
    Service(ServiceArgs),
    #[clap()]
    CI(CIArgs),
    #[clap(name="load-env", aliases = ["e", "env"])]
    LoadEnvironment(LoadEnvironmentArgs),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    if *DEBUG_MODE {
        info!("Debug mode enabled");
    }

    let args = SuiOpArgs::parse();
    match args.resource {
        Resource::Docker(args) => {
            docker_cmd(&args).await?;
        }
        Resource::Iam(args) => {
            iam_cmd(&args).await?;
        }
        Resource::Incidents(args) => {
            incidents_cmd(&args).await?;
        }
        Resource::Pulumi(args) => {
            pulumi_cmd(&args)?;
        }
        Resource::Service(args) => {
            service_cmd(&args).await?;
        }
        Resource::CI(args) => {
            ci_cmd(&args).await?;
        }
        Resource::LoadEnvironment(args) => {
            load_environment_cmd(&args)?;
        }
    }

    Ok(())
}
