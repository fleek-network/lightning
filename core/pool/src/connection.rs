use std::{
    collections::{HashMap, HashSet},
    io::Write,
    net::SocketAddr,
};

use affair::AsyncWorker;
use anyhow::{bail, Result};
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{schema::LightningMessage, types::ServiceScope};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot,
};

use crate::connector::ConnectEvent;

/// Driver for driving the connection events from the transport connection.
pub struct ListenerDriver {
    /// Current active connections.
    handles: HashMap<ServiceScope, Sender<(SendStream, RecvStream)>>,
    /// Listens for scoped service registration.
    register_rx: Receiver<RegisterEvent>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

pub async fn start_listener_driver<T: LightningMessage>(mut driver: ListenerDriver) {
    loop {
        tokio::select! {
            event = driver.register_rx.recv() => {
                let event = match event {
                    Some(event) => event,
                    None => break,
                };
                driver.handles.insert(event.scope, event.handle);
            }
            connecting = driver.endpoint.accept() => {
                let connecting = match connecting {
                    Some(connecting) => connecting,
                    None => break,
                };
                let connection = connecting.await.unwrap();
                let handles = driver.handles.clone();
                tokio::spawn(async move {
                    let mut stream = connection.accept_uni().await.unwrap();
                    let data = stream.read_to_end(4096).await.unwrap();
                    let message: ScopedMessage<T> = ScopedMessage::decode(&data).unwrap();
                    if let Some(handle) = handles.get(&message.scope) {
                        let bi_stream = connection.accept_bi().await.unwrap();
                        handle.send(bi_stream).await.unwrap();
                    }
                });
            }
        }
    }
}

/// Driver for driving the connection events from the transport connection.
pub struct ConnectorDriver {
    /// Listens for scoped service registration.
    connect_rx: Receiver<ConnectEvent>,
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

impl<T> LightningMessage for ScopedMessage<T>
where
    T: LightningMessage,
{
    fn decode(buffer: &[u8]) -> Result<Self> {
        todo!()
    }

    fn encode<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        todo!()
    }
}

/// Event created on `listen` and `connect`.
pub struct RegisterEvent {
    /// Whether we should remove this scope.
    close: bool,
    /// Scope to be registered.
    scope: ServiceScope,
    /// Handle to send back stream.
    handle: Sender<(SendStream, RecvStream)>,
}
