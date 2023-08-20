use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::{AutoImplSerde, LightningMessage};
use lightning_interfaces::types::ServiceScope;
use quinn::{ClientConfig, Connection, Endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use crate::connector::ConnectEvent;
use crate::netkit;
use crate::pool::ScopeHandle;

pub async fn start_listener_driver(driver: ListenerDriver) {
    while let Some(connecting) = driver.endpoint.accept().await {
        let remote_address = connecting.remote_address();
        let connection = match connecting.await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("failed to establish QUIC connection to {remote_address}: {e:?}");
                continue;
            },
        };

        tracing::info!("connection established {}", connection.remote_address());

        let handles = driver.handles.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_incoming_connection(handles, connection).await {
                tracing::error!("failed to handle incoming connection: {e:?}");
            }
        });
    }
}

async fn handle_incoming_connection(
    handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    connection: Connection,
) -> Result<()> {
    let (tx, mut rx) = connection.accept_bi().await?;
    let data = rx.read_to_end(1024).await?;
    let message: StreamRequest = StreamRequest::decode(&data)?;
    if let Some(handle) = handles.get(&message.scope) {
        if handle
            .listener_tx
            .send((message.source_peer, tx, rx))
            .await
            .is_err()
        {
            tracing::error!("listener dropped the channel");
        }
    }
    Ok(())
}

pub async fn start_connector_driver(mut driver: ConnectorDriver) {
    while let Some(event) = driver.connect_rx.recv().await {
        match driver.pool.get(&(event.pk, event.address)) {
            None => {
                let endpoint = driver.endpoint.clone();
                let pool = driver.pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_new_outgoing_connection(endpoint, event, pool).await {
                        tracing::error!("failed to handle outgoing connection: {e:?}")
                    }
                });
            },
            Some(connection) => {
                let connection = connection.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_existing_outgoing_connection(connection, event).await {
                        tracing::error!("failed to handle outgoing connection: {e:?}")
                    }
                });
            },
        };
    }
}

async fn handle_new_outgoing_connection(
    endpoint: Endpoint,
    event: ConnectEvent,
    pool: Arc<DashMap<(NodePublicKey, SocketAddr), Connection>>,
) -> Result<()> {
    let config = netkit::client_config();
    let client_config = ClientConfig::new(Arc::new(config));
    let connection = endpoint
        .connect_with(client_config, event.address, "")?
        .await?;
    pool.insert((event.pk, event.address), connection.clone());

    handle_existing_outgoing_connection(connection, event).await
}

async fn handle_existing_outgoing_connection(
    connection: Connection,
    event: ConnectEvent,
) -> Result<()> {
    let (mut tx, rx) = connection.open_bi().await?;

    let mut writer = Vec::with_capacity(1024);
    LightningMessage::encode::<Vec<_>>(
        &StreamRequest {
            source_peer: event.pk,
            scope: event.scope,
        },
        writer.as_mut(),
    )?;
    let _ = tx.write(writer.as_mut()).await?;

    if event.respond.send((tx, rx)).is_err() {
        tracing::error!("connector dropped the channel");
    }
    Ok(())
}

/// Driver for driving the connection events from the transport connection.
pub struct ListenerDriver {
    /// Current active connections.
    handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

impl ListenerDriver {
    pub fn new(handles: Arc<DashMap<ServiceScope, ScopeHandle>>, endpoint: Endpoint) -> Self {
        Self { handles, endpoint }
    }
}

/// Driver for driving the connection events from the transport connection.
pub struct ConnectorDriver {
    /// Listens for scoped service registration.
    connect_rx: Receiver<ConnectEvent>,
    /// QUIC connection pool.
    pool: Arc<DashMap<(NodePublicKey, SocketAddr), Connection>>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

impl ConnectorDriver {
    pub fn new(connect_rx: Receiver<ConnectEvent>, endpoint: Endpoint) -> Self {
        Self {
            connect_rx,
            pool: Arc::new(DashMap::new()),
            endpoint,
        }
    }
}

/// Request use for establishing new stream connection.
#[derive(Deserialize, Serialize)]
pub struct StreamRequest {
    scope: ServiceScope,
    source_peer: NodePublicKey,
}

impl AutoImplSerde for StreamRequest {}
