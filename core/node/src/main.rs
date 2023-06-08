mod cli;
mod configuration;
mod template;

use clap::Parser;
use draco_application::app::Application;
use draco_handshake::server::{StreamProvider, TcpHandshakeServer, TcpProvider};
use draco_interfaces::{common::WithStartAndShutdown as _, Node};
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
    Rpc,
    Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    TcpHandshakeServer<
        Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    >,
>;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    println!("Hello, Fleek!");

    let config = TomlConfigProvider::open(args.config).unwrap();
    let node = ConcreteNode::init(config).await.unwrap();

    node.start().await;

    // TODO: Await ctrl^c/sigkill and call shutdown
}
