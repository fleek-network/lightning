use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use affair::AsyncWorker;
use anyhow::{bail, Result};
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::connector::ConnectEvent;
use crate::netkit;

pub async fn start_listener_driver(mut driver: ListenerDriver) {
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
                    let (tx, mut rx) = connection.accept_bi().await.unwrap();
                    let data = rx.read_to_end(4096).await.unwrap();
                    let message: ScopedMessage = ScopedMessage::decode(&data).unwrap();
                    if let Some(handle) = handles.get(&message.scope) {
                        handle.send((tx, rx)).await.unwrap();
                    }
                });
            }
        }
    }
}

pub async fn start_connector_driver<T: LightningMessage>(mut driver: ConnectorDriver) {
    while let Some(event) = driver.connect_rx.recv().await {
        let connection = match driver.pool.get(&(event.pk, event.address)) {
            None => {
                let config = netkit::client_config();
                let client_config = ClientConfig::new(Arc::new(config));
                let connection = driver
                    .endpoint
                    .connect_with(client_config, event.address, "")
                    .unwrap()
                    .await
                    .unwrap();
                driver
                    .pool
                    .insert((event.pk, event.address), connection.clone());
                connection
            },
            Some(connection) => connection.clone(),
        };
        let (mut tx, mut rx) = connection.open_bi().await.unwrap();
        let mut writer = Vec::with_capacity(4096);

        LightningMessage::encode::<Vec<_>>(&ScopedMessage { scope: event.scope }, writer.as_mut())
            .unwrap();
        let _ = tx.write(writer.as_mut()).await.unwrap();
        event.respond.send((tx, rx)).unwrap();
    }
}

/// Driver for driving the connection events from the transport connection.
pub struct ListenerDriver {
    /// Current active connections.
    handles: HashMap<ServiceScope, Sender<(SendStream, RecvStream)>>,
    /// Listens for scoped service registration.
    register_rx: Receiver<RegisterEvent>,
    /// QUIC endpoint.
    endpoint: Endpoint,
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
pub struct ScopedMessage {
    /// Channel ID.
    scope: ServiceScope,
}

impl LightningMessage for ScopedMessage {
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
