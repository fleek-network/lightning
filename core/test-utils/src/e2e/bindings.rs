use lightning_application::Application;
use lightning_blockstore::blockstore::Blockstore;
use lightning_checkpointer::Checkpointer;
use lightning_committee_beacon::CommitteeBeaconComponent;
use lightning_interfaces::partial_node_components;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::Rpc;
use lightning_signer::Signer;
use lightning_topology::Topology;
use lightning_utils::config::TomlConfigProvider;

use super::SyncBroadcaster;
use crate::consensus::{MockConsensus, MockForwarder};
use crate::keys::EphemeralKeystore;

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
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
});
