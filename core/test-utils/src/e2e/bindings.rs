use lightning_application::Application;
use lightning_blockstore::blockstore::Blockstore;
use lightning_checkpointer::Checkpointer;
use lightning_committee_beacon::CommitteeBeaconComponent;
use lightning_consensus::consensus::Consensus;
use lightning_forwarder::Forwarder;
use lightning_interfaces::partial_node_components;
use lightning_notifier::Notifier;
use lightning_pinger::Pinger;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::Rpc;
use lightning_signer::Signer;
use lightning_topology::Topology;
use lightning_utils::config::TomlConfigProvider;

use super::SyncBroadcaster;
use crate::consensus::{MockConsensus, MockForwarder};
use crate::keys::EphemeralKeystore;

partial_node_components!(TestFullNodeComponentsWithRealConsensus {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = SyncBroadcaster<Self>;
    BlockstoreInterface = Blockstore<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    CommitteeBeaconInterface = CommitteeBeaconComponent<Self>;
    ConfigProviderInterface = TomlConfigProvider<Self>;
    ConsensusInterface = Consensus<Self>;
    ForwarderInterface = Forwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PingerInterface = Pinger<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
});

partial_node_components!(TestFullNodeComponentsWithMockConsensus {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = SyncBroadcaster<Self>;
    BlockstoreInterface = Blockstore<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    CommitteeBeaconInterface = CommitteeBeaconComponent<Self>;
    ConfigProviderInterface = TomlConfigProvider<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PingerInterface = Pinger<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
});

partial_node_components!(TestFullNodeComponentsWithRealConsensusWithoutCommitteeBeacon {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = SyncBroadcaster<Self>;
    BlockstoreInterface = Blockstore<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    ConfigProviderInterface = TomlConfigProvider<Self>;
    ConsensusInterface = Consensus<Self>;
    ForwarderInterface = Forwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PingerInterface = Pinger<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
});

partial_node_components!(TestFullNodeComponentsWithoutCommitteeBeacon {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = SyncBroadcaster<Self>;
    BlockstoreInterface = Blockstore<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    ConfigProviderInterface = TomlConfigProvider<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PingerInterface = Pinger<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
});
