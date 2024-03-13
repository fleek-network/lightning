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
            .with(C::KeystoreInterface::infu_initialize_hack)
            .with(
                C::BlockstoreInterface::infu_initialize_hack
                    .on("_post", C::BlockstoreInterface::infu_post_hack),
            )
            .with(C::NotifierInterface::infu_initialize_hack)
            .with(
                <C::IndexerInterface as IndexerInterface<C>>::infu_initialize_hack
            )
            .with(
                C::BlockstoreServerInterface::infu_initialize_hack
                    .on("start", C::BlockstoreServerInterface::start.spawn()),
            )
            .with(
                <C::ApplicationInterface as ApplicationInterface<C>>::infu_initialize_hack
                    .on("start", |c: &C::ApplicationInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ApplicationInterface| {
                        block_on(c.shutdown())
                    }),
            )
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
                C::TopologyInterface::infu_initialize_hack
                    .on("start", |c: &C::TopologyInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::TopologyInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::ArchiveInterface::infu_initialize_hack
                    .on("start", |c: &C::ArchiveInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ArchiveInterface| block_on(c.shutdown())),
            )
            .with(C::ForwarderInterface::infu_initialize_hack)
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
                C::OriginProviderInterface::infu_initialize_hack
                    .on("start", |c: &C::OriginProviderInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::OriginProviderInterface| {
                        block_on(c.shutdown())
                    }),
            )
            // .with(
            //     C::DeliveryAcknowledgmentAggregatorInterface::infu_initialize_hack.on(
            //         "start",
            //         |c: &C::DeliveryAcknowledgmentAggregatorInterface| {
            //             h.block_on(c.start())
            //         },
            //     ),
            // )
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
            .with(
                C::ServiceExecutorInterface::infu_initialize_hack
                    .on("start", |c: &C::ServiceExecutorInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::ServiceExecutorInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::SignerInterface::infu_initialize_hack
                    .on("_post", C::SignerInterface::infu_post_hack)
                    .on("start", |c: &C::SignerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::SignerInterface| block_on(c.shutdown())),
            )
            .with(
                C::FetcherInterface::infu_initialize_hack
                    .on("start", |c: &C::FetcherInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::FetcherInterface| block_on(c.shutdown())),
            )
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
            )
            .with(
                C::PingerInterface::infu_initialize_hack
                    .on("start", |c: &C::PingerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::PingerInterface| block_on(c.shutdown())),
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
        let handle = tokio::spawn(async move { shutdown.shutdown().await });
        self.provider.trigger("shutdown");
        handle.await.unwrap();
    }

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(config_provider: &impl ConfigProviderInterface<T>) {
        config_provider.get::<C::BlockstoreServerInterface>();
        config_provider.get::<C::KeystoreInterface>();
        config_provider.get::<C::SignerInterface>();
        config_provider.get::<C::ApplicationInterface>();
        config_provider.get::<C::BlockstoreInterface>();
        config_provider.get::<C::BroadcastInterface>();
        config_provider.get::<C::TopologyInterface>();
        config_provider.get::<C::ArchiveInterface>();
        config_provider.get::<C::ForwarderInterface>();
        config_provider.get::<C::ConsensusInterface>();
        config_provider.get::<C::HandshakeInterface>();
        config_provider.get::<C::OriginProviderInterface>();
        config_provider.get::<C::DeliveryAcknowledgmentAggregatorInterface>();
        config_provider.get::<C::ReputationAggregatorInterface>();
        config_provider.get::<C::ResolverInterface>();
        config_provider.get::<C::RpcInterface>();
        config_provider.get::<C::ServiceExecutorInterface>();
        config_provider.get::<C::FetcherInterface>();
        config_provider.get::<C::PoolInterface>();
        config_provider.get::<C::PingerInterface>();
    }
}
