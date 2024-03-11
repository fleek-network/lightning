use std::marker::PhantomData;

use fdi::{DependencyGraph, MethodExt, Provider};
use futures::executor::block_on;
pub use infusion::c;
use infusion::collection;

use super::*;

// Define the collection of every top-level trait in the system.
collection!([
    ConfigProviderInterface,
    KeystoreInterface,
    ApplicationInterface,
    BlockStoreInterface,
    BlockStoreServerInterface,
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
    collection: PhantomData<C>,
}

impl<C: Collection> Node<C> {
    pub fn init(config: C::ConfigProviderInterface) -> Result<Self, infusion::InitializationError> {
        let graph = DependencyGraph::new()
            .with_infallible((|| config).with_display_name("Config_Provider"))
            .with(C::KeystoreInterface::infu_initialize_hack.with_display_name("Keystore"))
            .with(C::BlockStoreInterface::infu_initialize_hack.with_display_name("Blockstore"))
            .with(C::NotifierInterface::infu_initialize_hack.with_display_name("Notifier"))
            .with(
                <C::IndexerInterface as IndexerInterface<C>>::infu_initialize_hack
                    .with_display_name("Indexer"),
            )
            .with(
                C::BlockStoreServerInterface::infu_initialize_hack
                    .with_display_name("Blockstore_Server")
                    .on("start", |c: &C::BlockStoreServerInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::BlockStoreServerInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                <C::ApplicationInterface as ApplicationInterface<C>>::infu_initialize_hack
                    .with_display_name("Application")
                    .on("start", |c: &C::ApplicationInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ApplicationInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::SyncronizerInterface::infu_initialize_hack
                    .with_display_name("Syncronizer")
                    .on("start", |c: &C::SyncronizerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::SyncronizerInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::BroadcastInterface::infu_initialize_hack
                    .with_display_name("Broadcast")
                    .on("start", |c: &C::BroadcastInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::BroadcastInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::TopologyInterface::infu_initialize_hack
                    .with_display_name("Topology")
                    .on("start", |c: &C::TopologyInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::TopologyInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::ArchiveInterface::infu_initialize_hack
                    .with_display_name("Archive")
                    .on("start", |c: &C::ArchiveInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ArchiveInterface| block_on(c.shutdown())),
            )
            .with(C::ForwarderInterface::infu_initialize_hack.with_display_name("Forwarder"))
            .with(
                C::ConsensusInterface::infu_initialize_hack
                    .with_display_name("Consensus")
                    .on("start", |c: &C::ConsensusInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ConsensusInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::HandshakeInterface::infu_initialize_hack
                    .with_display_name("Handshake")
                    .on("start", |c: &C::HandshakeInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::HandshakeInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::OriginProviderInterface::infu_initialize_hack
                    .with_display_name("Origin_Provider")
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
                    .with_display_name("Reputation_Aggregator")
                    .on("start", |c: &C::ReputationAggregatorInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::ReputationAggregatorInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                <C::ResolverInterface as ResolverInterface<C>>::infu_initialize_hack
                    .with_display_name("Resolver")
                    .on("start", |c: &C::ResolverInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::ResolverInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::RpcInterface::infu_initialize_hack
                    .with_display_name("Rpc")
                    .on("start", |c: &C::RpcInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::RpcInterface| block_on(c.shutdown())),
            )
            .with(
                C::ServiceExecutorInterface::infu_initialize_hack
                    .with_display_name("Service_Executor")
                    .on("start", |c: &C::ServiceExecutorInterface| {
                        block_on(c.start())
                    })
                    .on("shutdown", |c: &C::ServiceExecutorInterface| {
                        block_on(c.shutdown())
                    }),
            )
            .with(
                C::SignerInterface::infu_initialize_hack
                    .with_display_name("Signer")
                    .on("_post", C::SignerInterface::infu_post_hack)
                    .on("start", |c: &C::SignerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::SignerInterface| block_on(c.shutdown())),
            )
            .with(
                C::FetcherInterface::infu_initialize_hack
                    .with_display_name("Fetcher")
                    .on("start", |c: &C::FetcherInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::FetcherInterface| block_on(c.shutdown())),
            )
            .with(
                C::PoolInterface::infu_initialize_hack
                    .with_display_name("Pool")
                    .on("start", |c: &C::PoolInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::PoolInterface| block_on(c.shutdown())),
            )
            .with(
                C::PingerInterface::infu_initialize_hack
                    .with_display_name("Pinger")
                    .on("start", |c: &C::PingerInterface| block_on(c.start()))
                    .on("shutdown", |c: &C::PingerInterface| block_on(c.shutdown())),
            );

        let vis = graph.viz("Lightning Dependency Graph");
        println!("{vis}");

        let mut container = Provider::default();
        graph
            .init_all(&mut container)
            .expect("failed to init dependency graph");

        Ok(Self {
            provider: container,
            collection: PhantomData,
        })
    }

    pub async fn start(&self) {
        self.provider.trigger("start");
    }

    /// Temporary shutdown for old start and shutdown handlers
    pub async fn shutdown(&self) {
        self.provider.trigger("shutdown");
    }

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(config_provider: &impl ConfigProviderInterface<T>) {
        config_provider.get::<C::BlockStoreServerInterface>();
        config_provider.get::<C::KeystoreInterface>();
        config_provider.get::<C::SignerInterface>();
        config_provider.get::<C::ApplicationInterface>();
        config_provider.get::<C::BlockStoreInterface>();
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
