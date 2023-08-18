use lightning_application::app::Application;
use lightning_blockstore::memory::MemoryBlockStore;
use lightning_broadcast::Broadcast;
use lightning_consensus::consensus::Consensus;
use lightning_dht::dht::Dht;
use lightning_handshake::server::TcpHandshakeServer;
use lightning_interfaces::infu_collection::Collection;
use lightning_notifier::Notifier;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::server::Rpc;
use lightning_signer::Signer;
use lightning_topology::Topology;

use crate::config::TomlConfigProvider;

/// Finalized type bindings for Lightning.
#[derive(Clone)]
pub struct FinalTypes;

impl Collection for FinalTypes {
    type ConfigProviderInterface = TomlConfigProvider<Self>;
    type ApplicationInterface = Application<Self>;
    type BlockStoreInterface = MemoryBlockStore<Self>;
    type BroadcastInterface = Broadcast<Self>;
    type ConnectionPoolInterface = infusion::Blank<Self>;
    type TopologyInterface = Topology<Self>;
    type ConsensusInterface = Consensus<Self>;
    type HandshakeInterface = TcpHandshakeServer<Self>;
    type NotifierInterface = Notifier<Self>;
    type OriginProviderInterface = infusion::Blank<Self>;
    type DeliveryAcknowledgmentAggregatorInterface = infusion::Blank<Self>;
    type ReputationAggregatorInterface = ReputationAggregator<Self>;
    type ResolverInterface = infusion::Blank<Self>;
    type RpcInterface = Rpc<Self>;
    type DhtInterface = Dht<Self>;
    type ServiceExecutorInterface = infusion::Blank<Self>;
    type SignerInterface = Signer<Self>;
}
