use infusion::collection;

use super::*;

// Define the collection of every top-level trait in the system.
collection!([
    ConfigProviderInterface,
    ApplicationInterface,
    BlockStoreInterface,
    BroadcastInterface,
    ConnectionPoolInterface,
    TopologyInterface,
    ConsensusInterface,
    HandshakeInterface,
    NotifierInterface,
    OriginProviderInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    ReputationAggregatorInterface,
    ResolverInterface,
    RpcInterface,
    DhtInterface,
    ServiceExecutorInterface,
    SignerInterface
]);
