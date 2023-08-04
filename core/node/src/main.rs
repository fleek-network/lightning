mod cli;
mod config;
mod node;
mod shutdown;
mod template;

use std::fs::File;

use anyhow::Result;
use chrono::Local;
use clap::Parser;
use cli::Cli;
use lightning_interfaces::{
    transformers, ApplicationInterface, BroadcastInterface, LightningTypes,
};
use log::LevelFilter;
use mock::consensus::MockConsensus;
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
};

use crate::{cli::CliArgs, node::FinalTypes};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    let log_level = args.verbose;
    let log_filter = match log_level {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _3_or_more => LevelFilter::Trace,
    };

    // Add ignore for process subdag because Narwhal prints it as an err everytime it successfully
    // processes a new sub_dag
    let logger_config = ConfigBuilder::new()
        .add_filter_ignore_str("narwhal_consensus::bullshark")
        .add_filter_ignore_str("anemo")
        .set_target_level(LevelFilter::Error)
        .set_location_level(if args.log_location {
            LevelFilter::Error
        } else {
            LevelFilter::Trace
        })
        .build();

    let date = Local::now();
    let log_file = std::env::temp_dir().join(format!(
        "lightning-{}.log",
        date.format("%Y-%m-%d-%H:%M:%S")
    ));

    CombinedLogger::init(vec![
        TermLogger::new(
            log_filter,
            logger_config,
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            simplelog::Config::default(),
            File::create(log_file).unwrap(),
        ),
    ])
    .unwrap();

    if args.with_mock_consensus {
        log::info!("Using MockConsensus");

        type Node = transformers::WithConsensus<
            FinalTypes,
            MockConsensus<
                <<FinalTypes as LightningTypes>::Application as ApplicationInterface>::SyncExecutor,
                <<FinalTypes as LightningTypes>::Broadcast as BroadcastInterface>::PubSub<()>,
            >,
        >;

        Cli::<Node>::new(args).exec().await
    } else {
        Cli::<FinalTypes>::new(args).exec().await
    }
}
