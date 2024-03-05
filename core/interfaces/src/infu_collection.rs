use std::marker::PhantomData;

use fdi::{DependencyGraph, MethodExt, Registry};
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
    // pub container: infusion::Container,
    pub container: Registry,
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
                    }),
            )
            .with(
                <C::ApplicationInterface as ApplicationInterface<C>>::infu_initialize_hack
                    .with_display_name("Application")
                    .on("start", |c: &C::ApplicationInterface| block_on(c.start())),
            )
            .with(
                C::SyncronizerInterface::infu_initialize_hack
                    .with_display_name("Syncronizer")
                    .on("start", |c: &C::SyncronizerInterface| block_on(c.start())),
            )
            .with(
                C::BroadcastInterface::infu_initialize_hack
                    .with_display_name("Broadcast")
                    .on("start", |c: &C::BroadcastInterface| block_on(c.start())),
            )
            .with(
                C::TopologyInterface::infu_initialize_hack
                    .with_display_name("Topology")
                    .on("start", |c: &C::TopologyInterface| block_on(c.start())),
            )
            .with(
                C::ArchiveInterface::infu_initialize_hack
                    .with_display_name("Archive")
                    .on("start", |c: &C::ArchiveInterface| block_on(c.start())),
            )
            .with(C::ForwarderInterface::infu_initialize_hack.with_display_name("Forwarder"))
            .with(
                C::ConsensusInterface::infu_initialize_hack
                    .with_display_name("Consensus")
                    .on("start", |c: &C::ConsensusInterface| block_on(c.start())),
            )
            .with(
                C::HandshakeInterface::infu_initialize_hack
                    .with_display_name("Handshake")
                    .on("start", |c: &C::HandshakeInterface| block_on(c.start())),
            )
            .with(
                C::OriginProviderInterface::infu_initialize_hack
                    .with_display_name("Origin_Provider")
                    .on("start", |c: &C::OriginProviderInterface| {
                        block_on(c.start())
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
                    }),
            )
            .with(
                <C::ResolverInterface as ResolverInterface<C>>::infu_initialize_hack
                    .with_display_name("Resolver")
                    .on("start", |c: &C::ResolverInterface| block_on(c.start())),
            )
            .with(
                C::RpcInterface::infu_initialize_hack
                    .with_display_name("Rpc")
                    .on("start", |c: &C::RpcInterface| block_on(c.start())),
            )
            .with(
                C::ServiceExecutorInterface::infu_initialize_hack
                    .with_display_name("Service_Executor")
                    .on("start", |c: &C::ServiceExecutorInterface| {
                        block_on(c.start())
                    }),
            )
            .with(
                C::SignerInterface::infu_initialize_hack
                    .with_display_name("Signer")
                    .on("_post", C::SignerInterface::infu_post_hack)
                    .on("start", |c: &C::SignerInterface| block_on(c.start())),
            )
            .with(
                C::FetcherInterface::infu_initialize_hack
                    .with_display_name("Fetcher")
                    .on("start", |c: &C::FetcherInterface| block_on(c.start())),
            )
            .with(
                C::PoolInterface::infu_initialize_hack
                    .with_display_name("Pool")
                    .on("start", |c: &C::PoolInterface| block_on(c.start())),
            )
            .with(
                C::PingerInterface::infu_initialize_hack
                    .with_display_name("Pinger")
                    .on("start", |c: &C::PingerInterface| block_on(c.start())),
            );

        let vis = graph.viz("Lightning Dependency Graph");
        println!("{vis}");

        let mut container = Registry::default();
        graph
            .init_all(&mut container)
            .expect("failed to init dependency graph");

        Ok(Self {
            container,
            collection: PhantomData,
        })
    }

    pub async fn start(&self) {
        self.container.trigger("start");
    }

    /// Temporary shutdown for old start and shutdown handlers
    pub async fn shutdown(&self) {
        self.container.trigger("shutdown");
    }

    /// Will load the appstate from a checkpoint. Stops the proccesses that depend on the appstate,
    /// replaces the db with the checkpoint and restarts the processess
    pub async fn load_checkpoint(&self, _checkpoint: ()) {
        // shutdown node
        // start_or_shutdown_node::<C>(&self.container, false).await;
        // load db

        // start_or_shutdown_node::<C>(&self.container, true).await;
        todo!("this should be refactored elsewhere, node should get dropped")
    }

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(provider: &impl ConfigProviderInterface<T>) {
        provider.get::<C::BlockStoreServerInterface>();
        provider.get::<C::KeystoreInterface>();
        provider.get::<C::SignerInterface>();
        provider.get::<C::ApplicationInterface>();
        provider.get::<C::BlockStoreInterface>();
        provider.get::<C::BroadcastInterface>();
        provider.get::<C::TopologyInterface>();
        provider.get::<C::ArchiveInterface>();
        provider.get::<C::ForwarderInterface>();
        provider.get::<C::ConsensusInterface>();
        provider.get::<C::HandshakeInterface>();
        provider.get::<C::OriginProviderInterface>();
        provider.get::<C::DeliveryAcknowledgmentAggregatorInterface>();
        provider.get::<C::ReputationAggregatorInterface>();
        provider.get::<C::ResolverInterface>();
        provider.get::<C::RpcInterface>();
        provider.get::<C::ServiceExecutorInterface>();
        provider.get::<C::FetcherInterface>();
        provider.get::<C::PoolInterface>();
        provider.get::<C::PingerInterface>();
    }
}
