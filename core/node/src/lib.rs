pub mod config;

use lightning_application::app::Application;
use lightning_archive::archive::Archive;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore_server::BlockStoreServer;
use lightning_broadcast::Broadcast;
use lightning_consensus::consensus::Consensus;
use lightning_fetcher::fetcher::Fetcher;
use lightning_handshake::handshake::Handshake;
use lightning_indexer::Indexer;
use lightning_interfaces::infu_collection::{
    Collection,
    CollectionBase,
    ConsensusInterfaceContainer,
    ConsensusInterfaceModifier,
};
use lightning_notifier::Notifier;
use lightning_origin_ipfs::IPFSOrigin;
use lightning_pinger::Pinger;
use lightning_pool::Pool;
use lightning_rep_collector::ReputationAggregator;
use lightning_resolver::resolver::Resolver;
use lightning_rpc::Rpc;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_signer::Signer;
use lightning_syncronizer::syncronizer::Syncronizer;
use lightning_topology::Topology;
use mock::consensus::MockConsensus;

use crate::config::TomlConfigProvider;

/// Finalized type bindings for Lightning.
#[derive(Clone)]
pub struct FinalTypes;

impl CollectionBase for FinalTypes {
    type ConfigProviderInterface<C: Collection> = TomlConfigProvider<C>;
    type ApplicationInterface<C: Collection> = Application<C>;
    type BlockStoreInterface<C: Collection> = Blockstore<C>;
    type BlockStoreServerInterface<C: Collection> = BlockStoreServer<C>;
    type SyncronizerInterface<C: Collection> = Syncronizer<C>;
    type BroadcastInterface<C: Collection> = Broadcast<C>;
    type TopologyInterface<C: Collection> = Topology<C>;
    type ArchiveInterface<C: Collection> = Archive<C>;
    type ConsensusInterface<C: Collection> = Consensus<C>;
    type HandshakeInterface<C: Collection> = Handshake<C>;
    type NotifierInterface<C: Collection> = Notifier<C>;
    type OriginProviderInterface<C: Collection> = IPFSOrigin<C>;
    type OriginFetcherInterface<C: Collection> =infusion::Blank<C>;
    type DeliveryAcknowledgmentAggregatorInterface<C: Collection> = infusion::Blank<C>;
    type ReputationAggregatorInterface<C: Collection> = ReputationAggregator<C>;
    type ResolverInterface<C: Collection> = Resolver<C>;
    type RpcInterface<C: Collection> = Rpc<C>;
    type ServiceExecutorInterface<C: Collection> = ServiceExecutor<C>;
    type SignerInterface<C: Collection> = Signer<C>;
    type FetcherInterface<C: Collection> = Fetcher<C>;
    type PoolInterface<C: Collection> = Pool<C>;
    type PingerInterface<C: Collection> = Pinger<C>;
    type IndexerInterface<C: Collection> = Indexer<C>;
}

// Create the collection modifier that can inject the mock consensus
// into the FinalTypes (or other collections.).

pub struct UseMockConsensusMarker;
impl ConsensusInterfaceContainer for UseMockConsensusMarker {
    type ConsensusInterface<C: Collection> = MockConsensus<C>;
}

pub type WithMockConsensus<O = FinalTypes> = ConsensusInterfaceModifier<UseMockConsensusMarker, O>;
