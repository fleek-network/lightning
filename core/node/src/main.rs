mod cli;
mod config;
mod shutdown;
mod template;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::Parser;
use draco_application::{app::Application, query_runner::QueryRunner};
use draco_consensus::consensus::Consensus;
use draco_handshake::server::{StreamProvider, TcpHandshakeServer, TcpProvider};
use draco_interfaces::{common::WithStartAndShutdown as _, ConfigProviderInterface, Node};
use draco_notifier::Notifier;
use draco_rep_collector::ReputationAggregator;
use draco_signer::Signer;
use template::{gossip::Gossip, topology::Topology};

use crate::{
    cli::{CliArgs, Command},
    config::TomlConfigProvider,
    shutdown::ShutdownController,
    template::{
        blockstore::BlockStore, fs::FileSystem, indexer::Indexer, origin::MyStream,
        pod::DeliveryAcknowledgmentAggregator, rpc::Rpc, sdk::Sdk,
    },
};

pub type ConcreteNode = Node<
    TomlConfigProvider,
    Consensus<QueryRunner, Gossip<Signer, Topology<QueryRunner>, Notifier>>,
    Application,
    BlockStore,
    Indexer,
    FileSystem,
    Signer,
    MyStream,
    DeliveryAcknowledgmentAggregator,
    Notifier,
    ReputationAggregator,
    Rpc<QueryRunner>,
    Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    TcpHandshakeServer<
        Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    >,
    Topology<QueryRunner>,
    Gossip<Signer, Topology<QueryRunner>, Notifier>,
>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.cmd {
        Command::Run => run(args.config).await,
        Command::PrintConfig { default } if default => print_default_config().await,
        Command::PrintConfig { .. } => print_config(args.config).await,
    }
}

/// Run the node with the provided configuration path.
async fn run(config_path: PathBuf) -> Result<()> {
    let shutdown_controller = ShutdownController::default();
    shutdown_controller.install_ctrl_c_handler();

    let config = Arc::new(load_or_write_config(config_path).await?);
    let node = ConcreteNode::init(config).await?;

    node.start().await;

    shutdown_controller.wait_for_shutdown().await;
    node.shutdown().await;

    Ok(())
}

/// Print the default configuration for the node, this function does not
/// create a new file.
async fn print_default_config() -> Result<()> {
    let config = TomlConfigProvider::default();
    ConcreteNode::fill_configuration(&config);
    println!("{}", config.serialize_config());
    Ok(())
}

/// Print the configuration from the given path.
async fn print_config(config_path: PathBuf) -> Result<()> {
    let config = load_or_write_config(config_path).await?;
    println!("{}", config.serialize_config());
    Ok(())
}

/// Load the configuration file and write the default to the disk.
async fn load_or_write_config(config_path: PathBuf) -> Result<TomlConfigProvider> {
    let config = TomlConfigProvider::open(&config_path)?;
    ConcreteNode::fill_configuration(&config);

    if !config_path.exists() {
        std::fs::write(&config_path, config.serialize_config())?;
    }

    Ok(config)
}
