use super::*;

// Define the node components of every top-level trait in the system. The node components is
// basically a trait with a bunch of associated types called members. Each of them taking the entire
// node components as input.
define_node_components!([
    ConfigProviderInterface,
    KeystoreInterface,
    ApplicationInterface,
    BlockstoreInterface,
    BlockstoreServerInterface,
    CheckpointerInterface,
    CommitteeBeaconInterface,
    SyncronizerInterface,
    BroadcastInterface,
    TopologyInterface,
    ArchiveInterface,
    ForwarderInterface,
    ConsensusInterface,
    HandshakeInterface,
    NotifierInterface,
    OriginProviderInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    ReputationAggregatorInterface,
    ResolverInterface,
    RpcInterface,
    ServiceExecutorInterface,
    TaskBrokerInterface,
    SignerInterface,
    FetcherInterface,
    PoolInterface,
    PingerInterface,
    IndexerInterface,
]);
