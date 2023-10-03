pub mod cli;
pub mod config;
pub mod node;
pub mod shutdown;
pub mod testnet_sync;

use std::process::exit;

use autometrics::settings::AutometricsSettingsBuilder;
use autometrics::{self};
use clap::Parser;
use cli::Cli;
use lightning_types::{DEFAULT_HISTOGRAM_BUCKETS, METRICS_SERVICE_NAME};

use crate::cli::CliArgs;
use crate::node::{FinalTypes, WithMockConsensus};

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    // init metrics exporter
    AutometricsSettingsBuilder::default()
        .service_name(METRICS_SERVICE_NAME)
        .histogram_buckets(DEFAULT_HISTOGRAM_BUCKETS)
        .init();

    let result = if args.with_mock_consensus {
        log::info!("Using MockConsensus");
        Cli::<WithMockConsensus>::new(args).exec().await
    } else {
        Cli::<FinalTypes>::new(args).exec().await
    };

    if let Err(err) = result {
        eprintln!("Command Failed.");
        eprintln!("Error: {err}");
        exit(-1);
    }
}
