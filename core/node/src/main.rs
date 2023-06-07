mod configuration;
mod template;

use std::{
    path::Path,
    task::{Context, Poll},
};

use draco_interfaces::{common::WithStartAndShutdown as _, Node};
use draco_rep_collector::ReputationAggregator;
use tokio::macros::support::Pin;
use tokio_stream::Stream;

use crate::{
    configuration::TomlConfigProvider,
    template::{
        application::Application, blockstore::BlockStore, consensus::Consensus, fs::FileSystem,
        handshake::Handshake, indexer::Indexer, pod::DeliveryAcknowledgmentAggregator, rpc::Rpc,
        sdk::Sdk, signer::Signer,
    },
};

pub struct MyStream {}

impl Stream for MyStream {
    type Item = bytes::BytesMut;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

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
    Sdk,
    Handshake,
>;

#[tokio::main]
async fn main() {
    println!("Hello, Fleek!");

    let path_str: String = String::from("node.toml");
    let from_string = Path::new(&path_str);
    let config: TomlConfigProvider = TomlConfigProvider::open(from_string).unwrap();
    let node: ConcreteNode = ConcreteNode::init(config).await.unwrap();

    node.start().await;
}
