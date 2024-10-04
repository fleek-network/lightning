//! # Preludes
//!
//! What should be included here?
//!
//! 1. All of the traits. This makes every method available and in scope leading to a better IDE
//!    support and dev ex.
//!
//! 2. Modules exported from top level.
//!
//! 3. Highly used macros, structs or enums that are used in almost all of the dependencies and are
//!    not only related to 1 particular component. Such as the `c` macro

// Re-export more traits from our lib dependencies.
pub use fdi::{Bind, BuildGraph, MethodExt};
pub use ink_quill::{ToDigest, TranscriptBuilder};
pub use lightning_schema::{AutoImplSerde, LightningMessage};

// Re-export top level modules and highly used stuff.
pub use crate::{c, fdi, partial_node_components, schema, spawn, types, ShutdownWaiter};

#[rustfmt::skip]
pub use crate::{
    ExecutionEngineSocket,
    OriginProviderSocket,
    SubmitTxSocket,
    FetcherSocket,
    DeliveryAcknowledgmentSocket,
    MempoolSocket,
    BlockstoreServerSocket
};

// Re-export all of the pub traits defined in our source code. Except the ones from our hack file.
//
// rg 'pub trait (\w+)' --only-matching --replace '$1,' -g '*.rs' -g '!_hacks.rs' \
//  --no-filename --no-line-number --no-heading ./src
#[rustfmt::skip]
pub use crate::{
    ApplicationInterface,
    ArchiveInterface,
    BlockstoreInterface,
    BlockstoreServerInterface,
    BroadcastEventInterface,
    BroadcastInterface,
    CheckpointerInterface,
    CheckpointerQueryInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    ConsensusInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    Emitter,
    EventHandlerInterface,
    ExecutorProviderInterface,
    FetcherInterface,
    ForwarderInterface,
    HandshakeInterface,
    IncrementalDirInterface,
    IncrementalPutInterface,
    IndexerInterface,
    KeystoreInterface,
    LaneManager,
    NodeComponents,
    NotifierInterface,
    OriginFinderAsyncIter,
    OriginProviderInterface,
    PingerInterface,
    PoolInterface,
    PubSub,
    ReputationAggregatorInterface,
    ReputationQueryInteface,
    ReputationReporterInterface,
    RequestInterface,
    RequesterInterface,
    ResolverInterface,
    ResponderInterface,
    ResponseInterface,
    RpcInterface,
    ServiceExecutorInterface,
    TaskBrokerInterface,
    SignerInterface,
    Subscriber,
    SyncQueryRunnerInterface,
    SyncronizerInterface,
    TopologyInterface,
    UntrustedStream,
};
