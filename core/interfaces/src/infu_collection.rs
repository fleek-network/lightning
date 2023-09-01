use std::marker::PhantomData;

pub use infusion::c;
use infusion::collection;

use super::*;

// Define the collection of every top-level trait in the system.
collection!([
    ConfigProviderInterface,
    ApplicationInterface,
    BlockStoreInterface,
    BlockStoreServerInterface,
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

    /// Fill the configuration provider with the default configuration without performing any
    /// initialization.
    pub fn fill_configuration<T: Collection>(provider: &impl ConfigProviderInterface<T>) {
        provider.get::<C::BlockStoreServerInterface>();
        provider.get::<C::SignerInterface>();
        provider.get::<C::ApplicationInterface>();
        provider.get::<C::BlockStoreInterface>();
        provider.get::<C::BroadcastInterface>();
        provider.get::<C::ConnectionPoolInterface>();
        provider.get::<C::TopologyInterface>();
        provider.get::<C::ConsensusInterface>();
        provider.get::<C::HandshakeInterface>();
        provider.get::<C::OriginProviderInterface>();
        provider.get::<C::DeliveryAcknowledgmentAggregatorInterface>();
        provider.get::<C::ReputationAggregatorInterface>();
        provider.get::<C::ResolverInterface>();
        provider.get::<C::RpcInterface>();
        provider.get::<C::DhtInterface>();
        provider.get::<C::ServiceExecutorInterface>();
        provider.get::<C::SignerInterface>();
    }
}

forward!(async fn start_or_shutdown_node(this, start: bool) on [
    BlockStoreServerInterface,
    SignerInterface,
    ApplicationInterface,
    ConnectionPoolInterface,
    BroadcastInterface,
    HandshakeInterface,
    ConsensusInterface,
    DhtInterface,
    ResolverInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    OriginProviderInterface,
    ServiceExecutorInterface,
    RpcInterface,
] {
    if start {
        log::info!("starting {}", get_name(&this));
        this.start().await;
    } else {
        log::info!("shutting down {}", get_name(&this));
        this.shutdown().await;
    }
});

fn get_name<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}
