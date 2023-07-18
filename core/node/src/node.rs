use draco_application::{app::Application, query_runner::QueryRunner};
use draco_blockstore::memory::MemoryBlockStore;
use draco_consensus::consensus::Consensus;
use draco_handshake::server::{StreamProvider, TcpHandshakeServer, TcpProvider};
use draco_interfaces::DracoTypes;
use draco_notifier::Notifier;
use draco_rep_collector::ReputationAggregator;
use draco_rpc::server::Rpc;
use draco_signer::Signer;

use crate::{
    config::TomlConfigProvider,
    template::{
        fs::FileSystem, gossip::Gossip, indexer::Indexer, origin::MyStream,
        pod::DeliveryAcknowledgmentAggregator, sdk::Sdk, topology::Topology,
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
