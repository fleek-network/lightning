pub mod config;

use lightning_application::app::Application;
use lightning_archive::archive::Archive;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore_server::BlockstoreServer;
use lightning_broadcast::Broadcast;
use lightning_consensus::consensus::Consensus;
use lightning_fetcher::fetcher::Fetcher;
use lightning_forwarder::Forwarder;
use lightning_handshake::handshake::Handshake;
use lightning_indexer::Indexer;
use lightning_interfaces::Collection;
use lightning_keystore::Keystore;
use lightning_notifier::Notifier;
use lightning_origin_demuxer::OriginDemuxer;
use lightning_pinger::Pinger;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_resolver::resolver::Resolver;
use lightning_rpc::Rpc;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_signer::Signer;
use lightning_syncronizer::syncronizer::Syncronizer;
use lightning_topology::Topology;

use crate::config::TomlConfigProvider;

/// Finalized type bindings for Lightning.
#[derive(Clone)]
pub struct FinalTypes;

impl Collection for FinalTypes {
    type ConfigProviderInterface = TomlConfigProvider<Self>;
    type ApplicationInterface = Application<Self>;
    type BlockstoreInterface = Blockstore<Self>;
    type BlockstoreServerInterface = BlockstoreServer<Self>;
    type SyncronizerInterface = Syncronizer<Self>;
    type BroadcastInterface = Broadcast<Self>;
    type TopologyInterface = Topology<Self>;
    type ArchiveInterface = Archive<Self>;
    type ForwarderInterface = Forwarder<Self>;
    type ConsensusInterface = Consensus<Self>;
    type HandshakeInterface = Handshake<Self>;
    type NotifierInterface = Notifier<Self>;
    type OriginProviderInterface = OriginDemuxer<Self>;
    type ReputationAggregatorInterface = ReputationAggregator<Self>;
    type ResolverInterface = Resolver<Self>;
    type RpcInterface = Rpc<Self>;
    type ServiceExecutorInterface = ServiceExecutor<Self>;
    type KeystoreInterface = Keystore<Self>;
    type SignerInterface = Signer<Self>;
    type FetcherInterface = Fetcher<Self>;
    type PoolInterface = PoolProvider<Self>;
    type PingerInterface = Pinger<Self>;
    type IndexerInterface = Indexer<Self>;
    type DeliveryAcknowledgmentAggregatorInterface = lightning_interfaces::_hacks::Blanket;
}

// Create the collection modifier that can inject the mock consensus
// into the FinalTypes (or other collections.).

pub type WithMockConsensus = FinalTypes;
