use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::mpsc::Receiver,
};

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::ServiceScope;
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use tokio::sync::{mpsc::Sender, oneshot};

/// Driver for driving the connection events from the transport connection.
pub struct ListenerDriver {
    /// Current active connections.
    active_scopes: HashSet<(ServiceScope, NodePublicKey), Sender<(SendStream, RecvStream)>>,
    /// Listens for scoped service registration.
    register_rx: Receiver<RegisterEvent>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

/// Driver for driving the connection events from the transport connection.
pub struct ConnectorDriver {
    /// Current active connections.
    active_scopes: HashSet<(ServiceScope, NodePublicKey)>,
    /// Listens for scoped service registration.
    register_rx: Receiver<RegisterEvent>,
    /// QUIC connection pool.
    pool: HashMap<(NodePublicKey, SocketAddr), Connection>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

/// Wrapper that allows us to create logical channels.
pub struct ScopedMessage<T> {
    /// Channel ID.
    scope: ServiceScope,
    /// The actual message.
    message: T,
}

/// Event created on `listen` and `connect`.
pub struct RegisterEvent {
    /// Whether we should remove this scope.
    close: bool,
    /// Scope to be registered.
    scope: ServiceScope,
    /// Handle to send back stream.
    handle: Option<oneshot::Sender<(SendStream, RecvStream)>>,
}
