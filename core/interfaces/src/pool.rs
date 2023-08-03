use async_trait::async_trait;
use derive_more::IsVariant;
use fleek_crypto::NodePublicKey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{ConfigConsumer, SyncQueryRunnerInterface, WithStartAndShutdown};

/// The protocol pre-defined connection scopes.
// Feel free to add more variant to the end on a need-to-have basis.
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize, IsVariant,
)]
#[repr(u8)]
pub enum ServiceScope {
    Dht,
    Broadcast,
}

/// Type alias for the return type of accept and connect.
pub type SenderReceiver<C, T> = (
    <C as ConnectionPool>::Sender<T>,
    <C as ConnectionPool>::Receiver<T>,
);

/// Type alias for the return type of bind.
pub type ListenerConnector<C, T> = (
    <C as ConnectionPool>::Listener<T>,
    <C as ConnectionPool>::Connector<T>,
);

/// The connection pool provides the basic functionality for a node to node communication.
///
/// Instances of this interface should never be input to other interfaces as initialization
/// parameter. However the [`ListenerConnector`] is a valid input. In other terms each interface
/// that is in need of connections should get a listener/connector for the scope that it
/// cares about during the node bootstrap life cycle.
pub trait ConnectionPool: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    // -- Generic input

    /// The query runner on the application to provide the information about a node
    /// to this connection pool. Should be a generic on this implementation.
    type QueryRunner: SyncQueryRunnerInterface;

    // -- Implementations provided by this interface.
    // The existence of Sender/Receiver at this layer and the link back to this connection
    // pool from listener and connector is to force the listener and connector to use the
    // same type for the sender/receiver.

    /// Listener object implemented by this connection pool.
    type Listener<T: Serialize + DeserializeOwned>: Listener<T, ConnectionPool = Self>;

    /// Connector object implemented by this connection pool.
    type Connector<T: Serialize + DeserializeOwned>: Connector<T, ConnectionPool = Self>;

    /// The sender struct used across the sender and connector.
    type Sender<T: Serialize + DeserializeOwned>: Sender<T>;

    /// The receiver struct used across the sender and connector.
    type Receiver<T: Serialize + DeserializeOwned>: Receiver<T>;

    /// Provide ownership to an scope in the connection pool.
    ///
    /// # Panics
    ///
    /// If this scope is already claimed.
    fn bind<T>(&self, scope: ServiceScope) -> (Self::Listener<T>, Self::Connector<T>)
    where
        T: Serialize + DeserializeOwned;
}

/// The connector can be used to connect to other peers under the scope.
pub trait Connector<T>: Send + Sync + Sized + Clone
where
    T: Serialize + DeserializeOwned,
{
    /// Link to the actual connection pool.
    type ConnectionPool: ConnectionPool;

    /// Create a new connection to the peer with the provided public key. Should return [`None`]
    /// if the connection pool is shutting down.
    fn connect(&self, to: &NodePublicKey) -> Option<SenderReceiver<Self::ConnectionPool, T>>;
}

/// The listener object
///
/// # Special consideration
///
/// The implementation of this struct has to provide a custom [`Drop`] implementation
/// in order to free the scope. So that successive calls to `bind` can succeed.
#[async_trait]
pub trait Listener<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Link to the actual connection pool.
    type ConnectionPool: ConnectionPool;

    /// Accept a new connection from a peer. Returns [`None`] if we are shutting down.
    async fn accept(&mut self) -> Option<SenderReceiver<Self::ConnectionPool, T>>;
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
pub trait Sender<T>: Send + Sync
where
    T: Serialize + DeserializeOwned,
{
    /// Return the public key associated with this connection.
    fn pk(&self) -> &NodePublicKey;

    /// Send a message to the destination. The future is resolved once we send the
    /// message.
    ///
    /// # Shutdown behavior
    ///
    /// If we are shutting down this function should return `false`.
    async fn send(&self, msg: &T) -> bool;
}

/// The scoped receiver owns an entire scope to itself and allows the holder to
/// receive messages in the scope.
///
/// # Special consideration
///
/// Drop implementation must perform graceful disconnection if there is no sender and receiver
/// object anymore.
#[async_trait]
pub trait Receiver<T>: Send + Sync
where
    T: Serialize + DeserializeOwned,
{
    /// Return the public key associated with this connection.
    fn pk(&self) -> &NodePublicKey;

    /// Receive a message. Returns [`None`] when the connection is closed.
    async fn recv(&mut self) -> Option<T>;
}
