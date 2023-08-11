use lightning_application::{app::Application, query_runner::QueryRunner};
use lightning_blockstore::memory::MemoryBlockStore;
use lightning_consensus::consensus::{Consensus, PubSubMsg};
use lightning_dht::dht::Dht;
use lightning_handshake::server::TcpHandshakeServer;
use lightning_interfaces::{BroadcastInterface, LightningTypes};
use lightning_notifier::Notifier;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::server::Rpc;
use lightning_signer::Signer;

use crate::{
    config::TomlConfigProvider,
    template::{
        broadcast::Broadcast, origin::MyStream, pod::DeliveryAcknowledgmentAggregator,
        pool::ConnectionPool, topology::Topology,
    },
};

/// Finalized type bindings for Lightning.
pub struct FinalTypes;

impl LightningTypes for FinalTypes {
    type ConfigProvider = TomlConfigProvider;
    type Consensus =
        Consensus<QueryRunner, <Self::Broadcast as BroadcastInterface>::PubSub<PubSubMsg>>;
    type Application = Application;
    type BlockStore = MemoryBlockStore;
    type Signer = Signer;
    type Stream = MyStream;
    type DeliveryAcknowledgmentAggregator = DeliveryAcknowledgmentAggregator;
    type Notifier = Notifier;
    type ReputationAggregator = ReputationAggregator;
    type Rpc = Rpc<QueryRunner>;
    type Handshake = TcpHandshakeServer;
    type Topology = Topology<QueryRunner>;
    type Broadcast = Broadcast<QueryRunner, Signer, Topology<QueryRunner>, Notifier>;
    type ConnectionPool = ConnectionPool<QueryRunner, Signer>;
    type Dht = Dht<Topology<QueryRunner>>;
}
