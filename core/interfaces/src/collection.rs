use std::marker::PhantomData;

use anyhow::Result;
use fdi::Provider;

use super::*;

// Define the collection of every top-level trait in the system. The collections is basically
// a trait with a bunch of associated types called members. Each of them taking the entire
// collection as input.
collection!([
    ConfigProviderInterface,
    KeystoreInterface,
    ApplicationInterface,
    BlockstoreInterface,
    BlockstoreServerInterface,
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
    SignerInterface,
    FetcherInterface,
    PoolInterface,
    PingerInterface,
    IndexerInterface,
]);

/// The Fleek Network node.
pub struct Node<C: Collection> {
    pub provider: Provider,
    pub shutdown: Option<ShutdownController>,
    _p: PhantomData<C>,
}

impl<C: Collection> Node<C> {
    #[inline(always)]
    pub fn init(config: C::ConfigProviderInterface) -> Result<Self> {
        let provider = Provider::default();
        provider.insert(config);
        Self::init_with_provider(provider)
    }

    pub fn init_with_provider(mut provider: Provider) -> Result<Self> {
        let mut exec = provider.get_mut::<fdi::Executor>();
        exec.set_spawn_cb(|fut| {
            tokio::spawn(fut);
        });

        let shutdown = ShutdownController::new();
        let waiter = shutdown.waiter();
        let graph = C::build_graph().with_value(waiter);

        let vis = graph.viz("Lightning Dependency Graph");
        println!("{vis}");

        graph
            .init_all(&mut provider)
            .expect("failed to init dependency graph");

        Ok(Self {
            provider,
            shutdown: Some(shutdown),
            _p: PhantomData,
        })
    }

    pub async fn start(&self) {
        self.provider.trigger("start");
    }

    /// Shutdown the node
    pub async fn shutdown(&mut self) {
        let mut shutdown = self
            .shutdown
            .take()
            .expect("cannot call shutdown more than once");

        shutdown.shutdown().await;
    }
}
