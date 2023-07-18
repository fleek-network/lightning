mod cli;
mod config;
mod shutdown;
mod template;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use draco_application::{app::Application, query_runner::QueryRunner};
use draco_blockstore::memory::MemoryBlockStore;
use draco_consensus::consensus::Consensus;
use draco_handshake::server::{StreamProvider, TcpHandshakeServer, TcpProvider};
use draco_interfaces::{transformers, ApplicationInterface, DracoTypes};
use draco_notifier::Notifier;
use draco_rep_collector::ReputationAggregator;
use draco_rpc::server::Rpc;
use draco_signer::Signer;
use mock::consensus::MockConsensus;
use template::{gossip::Gossip, topology::Topology};

use crate::{
    cli::CliArgs,
    config::TomlConfigProvider,
    template::{
        fs::FileSystem, indexer::Indexer, origin::MyStream, pod::DeliveryAcknowledgmentAggregator,
        sdk::Sdk,
    },
};

/// Finalized type bindings for Draco.
pub struct FinalTypes;

impl DracoTypes for FinalTypes {
    type ConfigProvider = TomlConfigProvider;
    type Consensus = Consensus<QueryRunner, Gossip<Signer, Topology<QueryRunner>, Notifier>>;
    type Application = Application;
    type BlockStore = MemoryBlockStore;
    type Indexer = Indexer;
    type FileSystem = FileSystem;
    type Signer = Signer;
    type Stream = MyStream;
    type DeliveryAcknowledgmentAggregator = DeliveryAcknowledgmentAggregator;
    type Notifier = Notifier;
    type ReputationAggregator = ReputationAggregator;
    type Rpc = Rpc<QueryRunner>;
    type Sdk =
        Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>;
    type Handshake = TcpHandshakeServer<
        Sdk<<TcpProvider as StreamProvider>::Reader, <TcpProvider as StreamProvider>::Writer>,
    >;
    type Topology = Topology<QueryRunner>;
    type Gossip = Gossip<Signer, Topology<QueryRunner>, Notifier>;
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    if args.with_mock_consensus {
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
