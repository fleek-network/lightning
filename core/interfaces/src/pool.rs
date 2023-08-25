use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use infusion::{c, ok};
use lightning_schema::LightningMessage;
use lightning_types::ServiceScope;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    ReputationAggregatorInterface,
    SignerInterface,
    WithStartAndShutdown,
};

/// Type alias for the return type of accept and connect.
pub type SenderReceiver<C, P, T> = (PoolSender<C, P, T>, PoolReceiver<C, P, T>);

/// Type alias for the return type of bind.
pub type ListenerConnector<C, P, T> = (
    <P as ConnectionPoolInterface<C>>::Listener<T>,
    <P as ConnectionPoolInterface<C>>::Connector<T>,
);

pub type PoolSender<C, P, T> =
    <<P as ConnectionPoolInterface<C>>::Listener<T> as ListenerInterface<T>>::Sender;
pub type PoolReceiver<C, P, T> =
    <<P as ConnectionPoolInterface<C>>::Listener<T> as ListenerInterface<T>>::Receiver;

/// The connection pool provides the basic functionality for a node to node communication.
///
/// Instances of this interface should never be input to other interfaces as initialization
/// parameter. However the [`ListenerConnector`] is a valid input. In other terms each interface
/// that is in need of connections should get a listener/connector for the scope that it
/// cares about during the node bootstrap life cycle.
#[infusion::service]
pub trait ConnectionPoolInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        signer: ::SignerInterface,
        app: ::ApplicationInterface,
        rep_aggregator: ::ReputationAggregatorInterface,
    ) {
        ok!(Self::init(
            config.get::<Self>(),
            signer,
            app.sync_query(),
            rep_aggregator.get_reporter()
        ))
    }

    /// Listener object implemented by this connection pool. The bounds here ensure
    /// that both listener and connector produce the same type.
    type Listener<T: LightningMessage>: ListenerInterface<
            T,
            Sender = <Self::Connector<T> as ConnectorInterface<T>>::Sender,
            Receiver = <Self::Connector<T> as ConnectorInterface<T>>::Receiver,
        > = infusion::Blank<T>;

    /// Connector object implemented by this connection pool.
    type Connector<T: LightningMessage>: ConnectorInterface<T> = infusion::Blank<T>;

    /// Initialize the pool with the given configuration.
    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    ) -> Self;

    /// Provide ownership to an scope in the connection pool.
    ///
    /// # Panics
    ///
    /// If this scope is already claimed.
    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: LightningMessage;
}

/// The connector can be used to connect to other peers under the scope.
#[async_trait]
#[infusion::blank]
pub trait ConnectorInterface<T>: Send + Sync + Sized + Clone
where
    T: LightningMessage,
{
    type Sender: SenderInterface<T> = infusion::Blank<T>;
    type Receiver: ReceiverInterface<T> = infusion::Blank<T>;

    /// Create a new connection to the peer with the provided public key. Should return [`None`]
    /// if the connection pool is shutting down.
    async fn connect(&self, to: &NodePublicKey) -> Option<(Self::Sender, Self::Receiver)>;
}

/// The listener object
///
/// # Special consideration
///
/// The implementation of this struct has to provide a custom [`Drop`] implementation
/// in order to free the scope. So that successive calls to `bind` can succeed.
#[async_trait]
#[infusion::blank]
pub trait ListenerInterface<T>: Send
where
    T: LightningMessage,
{
    type Sender: SenderInterface<T> = infusion::Blank<T>;
    type Receiver: ReceiverInterface<T> = infusion::Blank<T>;

    /// Accept a new connection from a peer. Returns [`None`] if we are shutting down.
    async fn accept(&mut self) -> Option<(Self::Sender, Self::Receiver)>;
}

/// An scoped sender allows the holder to send messages to other nodes through this connection.
/// This type *does not* 'own' the connection and connection must stay alive even if all senders
/// are dropped.
///
/// # Special consideration
///
/// Drop implementation must perform graceful disconnection if there is no sender and receiver
/// object anymore.
#[async_trait]
#[infusion::blank]
pub trait SenderInterface<T>: Send + Sync + 'static
where
    T: LightningMessage,
{
    /// Return the public key associated with this connection.
    fn pk(&self) -> &NodePublicKey;

    /// Send a message to the destination. The future is resolved once we send the
    /// message.
    ///
    /// # Shutdown behavior
    ///
    /// If we are shutting down this function should return `false`.
    async fn send(&self, msg: T) -> bool;
}

/// The scoped receiver owns an entire scope to itself and allows the holder to
/// receive messages in the scope.
///
/// # Special consideration
///
/// Drop implementation must perform graceful disconnection if there is no sender and receiver
/// object anymore.
#[async_trait]
#[infusion::blank]
pub trait ReceiverInterface<T>: Send + Sync + 'static
where
    T: LightningMessage,
{
    /// Return the public key associated with this connection.
    fn pk(&self) -> &NodePublicKey;

    /// Receive a message. Returns [`None`] when the connection is closed.
    async fn recv(&mut self) -> Option<T>;
}
