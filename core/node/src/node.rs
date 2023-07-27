use freek_application::{app::Application, query_runner::QueryRunner};
use freek_blockstore::memory::MemoryBlockStore;
use freek_consensus::consensus::{Consensus, PubSubMsg};
use freek_handshake::server::TcpHandshakeServer;
use freek_interfaces::{FreekTypes, GossipInterface};
use freek_notifier::Notifier;
use freek_rep_collector::ReputationAggregator;
use freek_rpc::server::Rpc;
use freek_signer::Signer;

use crate::{
    config::TomlConfigProvider,
    template::{
        fs::FileSystem, gossip::Gossip, indexer::Indexer, origin::MyStream,
        pod::DeliveryAcknowledgmentAggregator, topology::Topology,
    },
};

/// Finalized type bindings for Freek.
pub struct FinalTypes;

impl FreekTypes for FinalTypes {
    type ConfigProvider = TomlConfigProvider;
    type Consensus = Consensus<QueryRunner, <Self::Gossip as GossipInterface>::PubSub<PubSubMsg>>;
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
    type Handshake = TcpHandshakeServer;
    type Topology = Topology<QueryRunner>;
    type Gossip = Gossip<Signer, Topology<QueryRunner>, Notifier>;
}
