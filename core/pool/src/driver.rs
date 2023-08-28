use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use dashmap::DashMap;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::ReputationReporterInterface;
use quinn::{ClientConfig, Connection, Endpoint, TransportConfig};
use tokio::sync::mpsc::Receiver;

use crate::connector::ConnectEvent;
use crate::pool::ScopeHandle;
use crate::tls;

/// Task that listens for QUIC connections.
///
/// Sends new QUIC streams to pool listener.
pub async fn start_listener_driver<R>(driver: ListenerDriver<R>)
where
    R: ReputationReporterInterface + 'static,
{
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
        let reporter = driver.reporter.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_incoming_connection(handles, connection, reporter).await {
                tracing::error!("failed to handle incoming connection: {e:?}");
            }
        });

        connections.retain(|conn| conn.close_reason().is_none());
    }

    log::debug!("stopping the listener drivier");
}

async fn handle_incoming_connection<R: ReputationReporterInterface>(
    handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    connection: Connection,
    reporter: R,
) -> Result<()> {
    let (tx, mut rx) = connection.accept_bi().await?;

    let chain = connection
        .peer_identity()
        .expect("TLS handshake was successful")
        .downcast::<Vec<rustls::Certificate>>()
        .expect("TLS handshake was successful");
    let certificate = chain.first().expect("TLS handshake was successful");
    let source_peer = tls::parse_unverified(certificate.as_ref())
        .expect("TLS handshake was successful")
        .peer_pk();

    let mut buf = vec![0; 4];
    rx.read_exact(&mut buf).await?;
    let scope: ServiceScope = bincode::deserialize(&buf)?;

    // Report RTT observed during connection establishment.
    reporter.report_latency(&source_peer, connection.rtt());

    if let Some(handle) = handles.get(&scope) {
        if handle
            .listener_tx
            .send((source_peer, tx, rx))
            .await
            .is_err()
        {
            log::error!("listener dropped the channel");
        }
    }
    Ok(())
}

/// Task that listens for connect events and creates QUIC connections.
///
/// Sends new QUIC streams to pool connector.
pub async fn start_connector_driver<R>(mut driver: ConnectorDriver<R>)
where
    R: ReputationReporterInterface + 'static,
{
    while let Some(event) = driver.connect_rx.recv().await {
        let endpoint = driver.endpoint.clone();
        let pool = driver.pool.clone();
        let secret_key = driver.node_secret_key.clone();
        let reporter = driver.reporter.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_new_outgoing_connection(endpoint, event, pool, &secret_key, reporter).await
            {
                tracing::error!("failed to handle outgoing connection: {e:?}")
            }
        });
    }
}

async fn handle_new_outgoing_connection<R: ReputationReporterInterface>(
    endpoint: Endpoint,
    event: ConnectEvent,
    pool: Arc<DashMap<(NodePublicKey, SocketAddr), Connection>>,
    secret_key: &NodeSecretKey,
    reporter: R,
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

    // Report RTT observed during connection establishment.
    reporter.report_latency(&event.peer, connection.rtt());

    handle_existing_outgoing_connection(connection, event).await
}

async fn handle_existing_outgoing_connection(
    connection: Connection,
    event: ConnectEvent,
) -> Result<()> {
    let (mut tx, rx) = connection.open_bi().await?;
    let serialized_scope = bincode::serialize(&event.scope)?;
    let _ = tx.write(serialized_scope.as_slice()).await?;

    if event.respond.send((tx, rx)).is_err() {
        tracing::error!("connector dropped the channel");
    }
    Ok(())
}

/// Driver for listening to incoming QUIC connections.
pub struct ListenerDriver<R: ReputationReporterInterface> {
    /// Current active connections.
    handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    /// QUIC endpoint.
    endpoint: Endpoint,
    /// Reputation reporter.
    reporter: R,
}

impl<R: ReputationReporterInterface> ListenerDriver<R> {
    pub fn new(
        handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
        endpoint: Endpoint,
        reporter: R,
    ) -> Self {
        Self {
            handles,
            endpoint,
            reporter,
        }
    }
}

/// Driver for making QUIC connections.
pub struct ConnectorDriver<R> {
    /// Listens for scoped service registration.
    connect_rx: Receiver<ConnectEvent>,
    /// QUIC connection pool.
    pool: Arc<DashMap<(NodePublicKey, SocketAddr), Connection>>,
    /// QUIC endpoint.
    endpoint: Endpoint,
    /// Node's secret key.
    node_secret_key: NodeSecretKey,
    /// Reputation reporter.
    reporter: R,
}

impl<R: ReputationReporterInterface> ConnectorDriver<R> {
    pub fn new(
        connect_rx: Receiver<ConnectEvent>,
        endpoint: Endpoint,
        node_secret_key: NodeSecretKey,
        reporter: R,
    ) -> Self {
        Self {
            connect_rx,
            pool: Arc::new(DashMap::new()),
            endpoint,
            node_secret_key,
            reporter,
        }
    }
}
