mod cli;
mod configuration;
mod template;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use draco_application::{app::Application, query_runner::QueryRunner};
use draco_handshake::server::{StreamProvider, TcpHandshakeServer, TcpProvider};
use draco_interfaces::{common::WithStartAndShutdown as _, ConfigProviderInterface, Node};
use draco_rep_collector::ReputationAggregator;

use crate::{
    cli::CliArgs,
    configuration::TomlConfigProvider,
    template::{
        blockstore::BlockStore, consensus::Consensus, fs::FileSystem, indexer::Indexer,
        origin::MyStream, pod::DeliveryAcknowledgmentAggregator, rpc::Rpc, sdk::Sdk,
        signer::Signer,
    },
};

pub type ConcreteNode = Node<
    TomlConfigProvider,
    Consensus,
    Application,
    BlockStore,
    Indexer,
    FileSystem,
    Signer,
    MyStream,
    DeliveryAcknowledgmentAggregator,
    ReputationAggregator,
    Rpc<QueryRunner>,
    Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    TcpHandshakeServer<
        Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    >,
>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    println!("Hello, Fleek!");

    let config = Arc::new(TomlConfigProvider::open(args.config.clone())?);
    let node = ConcreteNode::init(config.clone()).await?;
    std::fs::write(args.config, config.serialize_config())?;

    node.start().await;

    // TODO: Await ctrl^c/sigkill and call shutdown

    Ok(())
}
