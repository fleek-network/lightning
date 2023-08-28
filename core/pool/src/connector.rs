use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::{ConnectorInterface, SyncQueryRunnerInterface};
use multiaddr::{Multiaddr, Protocol};
use quinn::{RecvStream, SendStream};
use tokio::sync::{mpsc, oneshot};

use crate::pool::ScopeHandle;
use crate::receiver::Receiver;
use crate::sender::Sender;

/// Connect event sent by connector.
pub struct ConnectEvent {
    /// The scope that the connector belongs to.
    pub scope: ServiceScope,
    /// The peer we are trying to connect to.
    pub peer: NodePublicKey,
    /// The address of the peer we want to connect to.
    pub address: SocketAddr,
    /// Receiver for getting back a QUIC stream.
    pub respond: oneshot::Sender<(SendStream, RecvStream)>,
}

/// Pool connector.
pub struct Connector<Q, T> {
    /// The scope that the connector belongs to.
    scope: ServiceScope,
    /// Sender for connect events.
    connection_event_tx: mpsc::Sender<ConnectEvent>,
    /// Current active scopes. Used for updating when dropping.
    active_scope: Arc<DashMap<ServiceScope, ScopeHandle>>,
    /// Query runner.
    query_runner: Q,
    _marker: PhantomData<T>,
}

impl<Q, T> Connector<Q, T> {
    pub fn new(
        scope: ServiceScope,
        connection_event_tx: mpsc::Sender<ConnectEvent>,
        active_scope: Arc<DashMap<ServiceScope, ScopeHandle>>,
        query_runner: Q,
    ) -> Self {
        Self {
            scope,
            connection_event_tx,
            active_scope,
            query_runner,
            _marker: PhantomData,
        }
    }
}

impl<Q, T> Clone for Connector<Q, T>
where
    Q: SyncQueryRunnerInterface,
{
    fn clone(&self) -> Self {
        Self {
            scope: self.scope,
            connection_event_tx: self.connection_event_tx.clone(),
            active_scope: self.active_scope.clone(),
            query_runner: self.query_runner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Q, T> Drop for Connector<Q, T> {
    fn drop(&mut self) {
        let scope = self.scope;
        // Unwrap is safe here because we are currently in the scope.
        self.active_scope.get_mut(&scope).unwrap().connector_active = false;
    }
}

#[async_trait]
impl<Q, T> ConnectorInterface<T> for Connector<Q, T>
where
    T: LightningMessage,
    Q: SyncQueryRunnerInterface,
{
    type Sender = Sender<T>;
    type Receiver = Receiver<T>;
    async fn connect(&self, to: &NodePublicKey) -> Option<(Self::Sender, Self::Receiver)> {
        let (tx, rx) = oneshot::channel();
        let node_info = self.query_runner.get_node_info(to).or_else(|| {
            tracing::warn!("failed to get node info for {to:?}");
            None
        })?;

        let address = format!("{}:{}", node_info.domain, node_info.ports.pool)
            .parse()
            .ok()?;

        log::debug!("connecting to {to} on {address}");

        self.connection_event_tx
            .send(ConnectEvent {
                scope: self.scope,
                peer: *to,
                address,
                respond: tx,
            })
            .await
            .ok()?;
        let (tx_stream, rx_stream) = rx.await.ok()?;
        Some((Sender::new(tx_stream, *to), Receiver::new(rx_stream, *to)))
    }
}

#[allow(dead_code)]
fn get_socket_address(multi_address: Multiaddr) -> Option<SocketAddr> {
    let (mut host, mut port) = (None, None);
    for addr in multi_address.into_iter() {
        match addr {
            Protocol::Ip6(ip) => {
                host = Some(IpAddr::from(ip));
            },
            Protocol::Ip4(ip) => {
                host = Some(IpAddr::from(ip));
            },
            Protocol::Udp(p) => {
                port = Some(p);
            },
            _ => {},
        };
    }
    port = Some(port.unwrap_or(4200));
    if host.is_none() || port.is_none() {
        tracing::error!("failed to get address from {multi_address:?}");
        return None;
    }
    Some(SocketAddr::new(host.unwrap(), port.unwrap()))
}
