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
use draco_interfaces::{transformers, ApplicationInterface, DracoTypes};
use mock::consensus::MockConsensus;
use simplelog::*;

use crate::{cli::CliArgs, node::FinalTypes, template::indexer::Indexer};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    let log_level = args.verbose;
    let log_filter = match log_level {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _3_or_more => log::LevelFilter::Trace,
    };

    let date = Local::now();
    let log_file =
        std::env::temp_dir().join(format!("draco-{}.log", date.format("%Y-%m-%d-%H:%M:%S")));

    CombinedLogger::init(vec![
        TermLogger::new(
            log_filter,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            Config::default(),
            File::create(log_file).unwrap(),
        ),
    ])
    .unwrap();

    if args.with_mock_consensus {
        log::info!("Using MockConsensus");

        type Node = transformers::WithConsensus<
            FinalTypes,
            MockConsensus<
                <<FinalTypes as DracoTypes>::Application as ApplicationInterface>::SyncExecutor,
                <FinalTypes as DracoTypes>::Gossip,
            >,
        >;

        Cli::<Node>::new(args).exec().await
    } else {
        Cli::<FinalTypes>::new(args).exec().await
    }
}
