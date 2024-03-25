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
            .with_module::<C::SignerInterface>()
            .with_module::<C::BlockstoreInterface>()
            .with_module::<C::NotifierInterface>()
            .with_module::<C::PingerInterface>()
            .with_module::<C::FetcherInterface>()
            .with_module::<C::KeystoreInterface>()
            .with_module::<C::BlockstoreServerInterface>()
            .with_module::<C::ForwarderInterface>()
            .with_module::<C::TopologyInterface>()
            .with_module::<C::IndexerInterface>()
            .with_module::<C::OriginProviderInterface>()
            .with_module::<C::ServiceExecutorInterface>()
            // TODO: Refactor the rest of start/shutdown/inits:
            .with(
                <C::ApplicationInterface as ApplicationInterface<C>>::infu_initialize_hack
                    .on("start", |c: &C::ApplicationInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ApplicationInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with_infallible(|app: &C::ApplicationInterface| {
                application::ApplicationInterface::sync_query(app)
            })
            .with(
                C::SyncronizerInterface::infu_initialize_hack
                    .on("start", |c: &C::SyncronizerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::SyncronizerInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::BroadcastInterface::infu_initialize_hack
                    .on("start", |c: &C::BroadcastInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::BroadcastInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::ArchiveInterface::infu_initialize_hack
                    .on("start", |c: &C::ArchiveInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ArchiveInterface| block_on(c.shutdown())),
            )
            .with(
                C::ConsensusInterface::infu_initialize_hack
                    .on("start", |c: &C::ConsensusInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ConsensusInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::HandshakeInterface::infu_initialize_hack
                    .on("start", |c: &C::HandshakeInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::HandshakeInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::DeliveryAcknowledgmentAggregatorInterface::infu_initialize_hack.on(
                    "start",
                    |daa: &C::DeliveryAcknowledgmentAggregatorInterface| {
                        block_on(daa.start())
                    },
                ).on("shutdown", |daa: Consume<C::DeliveryAcknowledgmentAggregatorInterface>| {
                    block_on(daa.shutdown())
                }),
            )
            .with(
                C::ReputationAggregatorInterface::infu_initialize_hack
                    .on("start", |c: &C::ReputationAggregatorInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::ReputationAggregatorInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                <C::ResolverInterface as ResolverInterface<C>>::infu_initialize_hack
                    .on("start", |c: &C::ResolverInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ResolverInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::RpcInterface::infu_initialize_hack
                    .on("start", |c: &C::RpcInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::RpcInterface| block_on(c.shutdown())),
            )
            // .with(
            //     C::SignerInterface::infu_initialize_hack
            //         .on("_post", C::SignerInterface::infu_post_hack)
            //         .on("start", |c: &C::SignerInterface| block_on(c.start()))
            //         .on("shutdown", |c: &C::SignerInterface| block_on(c.shutdown())),
            // )
            .with(
                C::PoolInterface::infu_initialize_hack
                    .on("start", |c: &C::PoolInterface| block_on(c.start()))
                    .on(
                        "shutdown",
                        (|c: Consume<C::PoolInterface>, waiter: Cloned<ShutdownWaiter>| async move {
                            c.shutdown().await;
                            // hack: use waiter as a simple guard to hold off shutdown until this is complete.
                            drop(waiter);
                        }).spawn(),
                    ),
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

        shutdown.shutdown().await;

        println!("triggering legacy");
        self.provider.trigger("shutdown");
        println!("finished triggering legacy shutdown event");

        println!("triggering shutdown");

        println!("finished triggering shutdown controller");
    }

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(_config_provider: &impl ConfigProviderInterface<T>) {}
}
