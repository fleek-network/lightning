use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use dashmap::DashMap;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use lightning_interfaces::schema::{AutoImplSerde, LightningMessage};
use lightning_interfaces::types::ServiceScope;
use quinn::{ClientConfig, Connection, Endpoint, TransportConfig};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use crate::connector::ConnectEvent;
use crate::pool::ScopeHandle;
use crate::tls;

/// Task that listens for QUIC connections.
///
/// Sends new QUIC streams to pool listener.
pub async fn start_listener_driver(driver: ListenerDriver) {
    let mut connections = Vec::new();
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
        connections.push(connection.clone());

        let handles = driver.handles.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_incoming_connection(handles, connection).await {
                tracing::error!("failed to handle incoming connection: {e:?}");
            }
        });

        connections.retain(|conn| conn.close_reason().is_none());
    }
}

async fn handle_incoming_connection(
    handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    connection: Connection,
) -> Result<()> {
    let (tx, mut rx) = connection.accept_bi().await?;

    let mut data = vec![0; 75];
    rx.read_exact(&mut data).await?;
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

/// Task that listens for connect events and creates QUIC connections.
///
/// Sends new QUIC streams to pool connector.
pub async fn start_connector_driver(mut driver: ConnectorDriver) {
    while let Some(event) = driver.connect_rx.recv().await {
        let endpoint = driver.endpoint.clone();
        let pool = driver.pool.clone();
        let secret_key = driver.node_secret_key.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_new_outgoing_connection(endpoint, event, pool, &secret_key).await
            {
                tracing::error!("failed to handle outgoing connection: {e:?}")
            }
        });
    }
}

async fn handle_new_outgoing_connection(
    endpoint: Endpoint,
    event: ConnectEvent,
    pool: Arc<DashMap<(NodePublicKey, SocketAddr), Connection>>,
    secret_key: &NodeSecretKey,
) -> Result<()> {
    let config = tls::make_client_config(secret_key, Some(event.peer))?;
    let mut client_config = ClientConfig::new(Arc::new(config));
    let mut transport_config = TransportConfig::default();
    transport_config.max_idle_timeout(Duration::from_secs(30).try_into().ok());
    transport_config.keep_alive_interval(Some(Duration::from_secs(10)));
    client_config.transport_config(Arc::new(transport_config));

    let connection = endpoint
        .connect_with(client_config, event.address, "localhost")?
        .await?;
    pool.insert((event.peer, event.address), connection.clone());

    handle_existing_outgoing_connection(connection, event).await
}

async fn handle_existing_outgoing_connection(
    connection: Connection,
    event: ConnectEvent,
) -> Result<()> {
    let (mut tx, rx) = connection.open_bi().await?;

    // size of the stream request struct
    let mut writer = Vec::with_capacity(75);
    LightningMessage::encode::<Vec<_>>(
        &StreamRequest {
            source_peer: event.peer,
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

/// Driver for listening to incoming QUIC connections.
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

/// Driver for making QUIC connections.
pub struct ConnectorDriver {
    /// Listens for scoped service registration.
    connect_rx: Receiver<ConnectEvent>,
    /// QUIC connection pool.
    pool: Arc<DashMap<(NodePublicKey, SocketAddr), Connection>>,
    /// QUIC endpoint.
    endpoint: Endpoint,
    /// Node's secret key.
    node_secret_key: NodeSecretKey,
}

impl ConnectorDriver {
    pub fn new(
        connect_rx: Receiver<ConnectEvent>,
        endpoint: Endpoint,
        node_secret_key: NodeSecretKey,
    ) -> Self {
        Self {
            connect_rx,
            pool: Arc::new(DashMap::new()),
            endpoint,
            node_secret_key,
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
