use std::marker::PhantomData;

pub use infusion::c;
use infusion::collection;
use tracing::info;

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
    pub container: infusion::Container,
    collection: PhantomData<C>,
}

impl<C: Collection> Node<C> {
    pub fn init(
        config: c![C::ConfigProviderInterface],
    ) -> Result<Self, infusion::InitializationError> {
        let graph = C::build_graph();

        let container = infusion::Container::default()
            .with(infusion::tag!(C::ConfigProviderInterface), config)
            .initialize(graph)?;

        Ok(Self {
            container,
            collection: PhantomData,
        })
    }

    pub async fn start(&self) {
        start_or_shutdown_node::<C>(&self.container, true).await;
    }

    pub async fn shutdown(&self) {
        start_or_shutdown_node::<C>(&self.container, false).await;
    }

    /// Will load the appstate from a checkpoint. Stops the proccesses that depend on the appstate,
    /// replaces the db with the checkpoint and restarts the processess
    pub async fn load_checkpoint(&self, _checkpoint: ()) {
        // shutdown node
        start_or_shutdown_node::<C>(&self.container, false).await;
        // load db

        start_or_shutdown_node::<C>(&self.container, true).await;
    }

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(provider: &impl ConfigProviderInterface<T>) {
        provider.get::<C::BlockStoreServerInterface>();
        provider.get::<C::SignerInterface>();
        provider.get::<C::ApplicationInterface>();
        provider.get::<C::BlockStoreInterface>();
        provider.get::<C::BroadcastInterface>();
        provider.get::<C::TopologyInterface>();
        provider.get::<C::ArchiveInterface>();
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

forward!(async fn start_or_shutdown_node(this, start: bool) on [
    BlockStoreServerInterface,
    SignerInterface,
    PoolInterface,
    ApplicationInterface,
    SyncronizerInterface,
    ReputationAggregatorInterface,
    PingerInterface,
    BroadcastInterface,
    HandshakeInterface,
    ArchiveInterface,
    ConsensusInterface,
    ResolverInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    OriginProviderInterface,
    FetcherInterface,
    ServiceExecutorInterface,
    RpcInterface,
] {
    if start {
        info!("starting {}", get_name(&this));
        this.start().await;
    } else {
        info!("shutting down {}", get_name(&this));
        this.shutdown().await;
    }
});

fn get_name<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}
