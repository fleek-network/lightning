use std::marker::PhantomData;

use fdi::{DependencyGraph, MethodExt, Provider};
use futures::executor::block_on;
pub use infusion::c;
use infusion::{collection, Blank};

use super::*;

// Define the collection of every top-level trait in the system.
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
    pub fn init(config: C::ConfigProviderInterface) -> Result<Self, infusion::InitializationError> {
        let provider = Provider::default();
        provider.insert(config);
        Self::init_with_provider(provider)
    }

    pub fn init_with_provider(
        mut provider: Provider,
    ) -> Result<Self, infusion::InitializationError> {
        let mut exec = provider.get_mut::<fdi::Executor>();
        exec.set_spawn_cb(|fut| {
            tokio::spawn(fut);
        });

        let shutdown = ShutdownController::new();
        let waiter = shutdown.waiter();

        let graph = DependencyGraph::new()
            .with_value(waiter)
            .with_value(Blank::<C>::default())
            .with_module::<C::ApplicationInterface>()
            .with_module::<C::BroadcastInterface>()
            .with_module::<C::HandshakeInterface>()
            .with_module::<C::ConsensusInterface>()
            .with_module::<C::SignerInterface>()
            .with_module::<C::BlockstoreInterface>()
            .with_module::<C::NotifierInterface>()
            .with_module::<C::PingerInterface>()
            .with_module::<C::FetcherInterface>()
            .with_module::<C::KeystoreInterface>()
            .with_module::<C::PoolInterface>()
            .with_module::<C::BlockstoreServerInterface>()
            .with_module::<C::ForwarderInterface>()
            .with_module::<C::TopologyInterface>()
            .with_module::<C::IndexerInterface>()
            .with_module::<C::ResolverInterface>()
            .with_module::<C::OriginProviderInterface>()
            .with_module::<C::ServiceExecutorInterface>()
            .with_module::<C::DeliveryAcknowledgmentAggregatorInterface>()
            .with_module::<C::RpcInterface>()
            // TODO: Refactor the rest of start/shutdown/inits:
            .with(
                C::SyncronizerInterface::infu_initialize_hack
                    .on("start", |c: &C::SyncronizerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::SyncronizerInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::ArchiveInterface::infu_initialize_hack
                    .on("start", |c: &C::ArchiveInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ArchiveInterface| block_on(c.shutdown())),
            )
            .with(
                C::ReputationAggregatorInterface::infu_initialize_hack
                    .on("start", |c: &C::ReputationAggregatorInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::ReputationAggregatorInterface| {
                        block_on(c.shutdown())
                    }),
            );

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

        let fut = shutdown.shutdown();
        self.provider.trigger("shutdown");
        fut.await;
    }

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(_config_provider: &impl ConfigProviderInterface<T>) {}
}
