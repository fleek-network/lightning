use lightning_application::app::Application;
use lightning_blockstore::memory::MemoryBlockStore;
use lightning_broadcast::Broadcast;
use lightning_consensus::consensus::Consensus;
use lightning_dht::dht::Dht;
use lightning_handshake::handshake::Handshake;
use lightning_interfaces::infu_collection::{
    Collection,
    CollectionBase,
    ConsensusInterfaceContainer,
    ConsensusInterfaceModifier,
};
use lightning_notifier::Notifier;
use lightning_pool::pool::ConnectionPool;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::server::Rpc;
use lightning_signer::Signer;
use lightning_topology::Topology;
use mock::consensus::MockConsensus;

use crate::config::TomlConfigProvider;

/// Finalized type bindings for Lightning.
#[derive(Clone)]
pub struct FinalTypes;

impl CollectionBase for FinalTypes {
    type ConfigProviderInterface<C: Collection> = TomlConfigProvider<C>;
    type ApplicationInterface<C: Collection> = Application<C>;
    type BlockStoreInterface<C: Collection> = MemoryBlockStore<C>;
    type BroadcastInterface<C: Collection> = Broadcast<C>;
    type ConnectionPoolInterface<C: Collection> = ConnectionPool<C>;
    type TopologyInterface<C: Collection> = Topology<C>;
    type ConsensusInterface<C: Collection> = Consensus<C>;
    type HandshakeInterface<C: Collection> = Handshake<C>;
    type NotifierInterface<C: Collection> = Notifier<C>;
    type OriginProviderInterface<C: Collection> = infusion::Blank<C>;
    type DeliveryAcknowledgmentAggregatorInterface<C: Collection> = infusion::Blank<C>;
    type ReputationAggregatorInterface<C: Collection> = ReputationAggregator<C>;
    type ResolverInterface<C: Collection> = infusion::Blank<C>;
    type RpcInterface<C: Collection> = Rpc<C>;
    type DhtInterface<C: Collection> = Dht<C>;
    type ServiceExecutorInterface<C: Collection> = infusion::Blank<C>;
    type SignerInterface<C: Collection> = Signer<C>;
}

// Create the collection modifier that can inject the mock consensus
// into the FinalTypes (or other collections.).

pub struct UseMockConsensusMarker;
impl ConsensusInterfaceContainer for UseMockConsensusMarker {
    type ConsensusInterface<C: Collection> = MockConsensus<C>;
}

pub type WithMockConsensus<O = FinalTypes> = ConsensusInterfaceModifier<UseMockConsensusMarker, O>;
