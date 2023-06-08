mod configuration;
mod template;

use std::path::Path;

use draco_application::app::Application;
use draco_handshake::server::{StreamProvider, TcpHandshakeServer, TcpProvider};
use draco_interfaces::{common::WithStartAndShutdown as _, Node};
use draco_rep_collector::ReputationAggregator;

use crate::{
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
    println!("Hello, Fleek!");

    let path_str: String = String::from("node.toml");
    let from_string = Path::new(&path_str);
    let config: TomlConfigProvider = TomlConfigProvider::open(from_string).unwrap();
    let node = ConcreteNode::init(config).await.unwrap();

    node.start().await;

    // TODO: Await ctrl^c/sigkill and call shutdown
}
