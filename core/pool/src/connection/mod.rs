mod connector;
mod driver;

use std::collections::HashMap;
use std::net::SocketAddr;

use anyhow::Result;
use bytes::Bytes;
use connector::ConnectionResult;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, ServiceScope, SyncQueryRunnerInterface};
use quinn::{Connection, Endpoint, ServerConfig};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinSet;

use crate::connection::connector::Connector;
use crate::connection::driver::{Context, DriverRequest};
use crate::service::broadcast::{BroadcastRequest, BroadcastService, BroadcastTask, Message};
use crate::service::stream::{StreamRequest, StreamService};

/// Manager for the transport connections.
pub struct ConnectionPool<C: Collection, W, R> {
    /// Pool of transport connections.
    pool: HashMap<NodeIndex, Sender<DriverRequest>>,
    /// Used for getting peer information from state.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Network broadcast service.
    broadcast_service: BroadcastService<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>,
    /// Send handle to return to users as they register with the broadcast service.
    broadcast_service_tx:
        Sender<BroadcastRequest<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>>,
    /// Receive requests for a multiplexed stream.
    stream_service: StreamService<W, R>,
    /// Send handle to return to users as they register with the stream service.
    stream_service_tx: Sender<StreamRequest<W, R>>,
    /// Source of network topology.
    _topology: c![C::TopologyInterface],
    /// Receiver of events from a connection.
    _connection_event_rx: Receiver<ConnectionEvent>,
    /// Sender of events from a connection.
    connection_event_tx: Sender<ConnectionEvent>,
    /// Performs dial tasks.
    connector: Connector<C>,
    /// Pending outgoing requests.
    pending_task: HashMap<NodeIndex, Vec<DriverRequest>>,
    /// Used for sending outbound requests to drivers.
    driver: HashMap<NodeIndex, Sender<DriverRequest>>,
    /// Ongoing drivers.
    driver_set: JoinSet<NodeIndex>,
    // Todo: Make an interface to abstract Endpoint.
    // This will allow us to use other protocols besides QUIC.
    /// QUIC Endpoint.
    endpoint: Option<Endpoint>,
    /// QUIC server config.
    server_config: ServerConfig,
    /// Node socket address.
    address: SocketAddr,
}

impl<C, W, R> ConnectionPool<C, W, R>
where
    C: Collection,
    W: AsyncWrite + Send + 'static,
    R: AsyncRead + Send + 'static,
{
    pub fn new(
        topology: c!(C::TopologyInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        sk: NodeSecretKey,
        server_config: ServerConfig,
        address: SocketAddr,
    ) -> Self {
        let (connection_event_tx, connection_event_rx) = mpsc::channel(1024);
        let (broadcast_service_tx, broadcast_service_rx) = mpsc::channel(1024);
        let (stream_service_tx, stream_service_rx) = mpsc::channel(1024);

        Self {
            pool: HashMap::new(),
            sync_query: sync_query.clone(),
            broadcast_service: BroadcastService::new(broadcast_service_rx),
            broadcast_service_tx,
            stream_service: StreamService::new(stream_service_rx),
            stream_service_tx,
            _topology: topology,
            _connection_event_rx: connection_event_rx,
            connection_event_tx,
            connector: Connector::new(sync_query, sk),
            pending_task: HashMap::new(),
            driver: HashMap::new(),
            driver_set: JoinSet::new(),
            endpoint: None,
            server_config,
            address,
        }
    }

    pub fn register_broadcast_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (
        Sender<BroadcastRequest<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>>,
        Receiver<Bytes>,
    ) {
        let rx = self.broadcast_service.register(service_scope);
        (self.broadcast_service_tx.clone(), rx)
    }

    pub fn register_stream_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<StreamRequest<W, R>>, Receiver<(W, R)>) {
        let rx = self.stream_service.register(service_scope);
        (self.stream_service_tx.clone(), rx)
    }

    pub fn handle_broadcast_task(&mut self, task: BroadcastTask) -> Result<()> {
        match task {
            BroadcastTask::Send {
                service_scope,
                message,
                peers,
            } => {
                let (connected, not_connected) = match peers {
                    None => (self.pool.keys().copied().collect::<Vec<_>>(), vec![]),
                    Some(peers) => peers
                        .iter()
                        .partition(|index| self.pool.contains_key(index)),
                };

                let message = Message {
                    service: service_scope,
                    payload: message.to_vec(),
                };

                for index in not_connected {
                    let Some(pk) = self.sync_query.index_to_pubkey(index) else {
                        continue;
                    };
                    let Some(info) = self.sync_query.get_node_info(&pk) else {
                        continue;
                    };
                    let address = NodeAddress {
                        index,
                        pk,
                        socket_address: SocketAddr::from((info.domain, info.ports.pool)),
                    };

                    self.connector.enqueue_dial_task(
                        address,
                        self.endpoint.clone().expect("Endpoint to exist in state"),
                    )?;
                    self.enqueue_pending_request(index, DriverRequest::Message(message.clone()))
                }

                for index in connected {
                    let Some(driver_tx) = self.pool.get(&index).cloned() else {
                        continue;
                    };
                    let request = DriverRequest::Message(message.clone());
                    tokio::spawn(async move {
                        if driver_tx.send(request).await.is_err() {
                            tracing::error!("failed to send driver an outgoing broadcast request");
                        }
                    });
                }
            },
            BroadcastTask::Update { .. } => {},
        }

        Ok(())
    }

    fn handle_connection(&mut self, peer: NodeIndex, connection: Connection, incoming: bool) {
        self.connector.cancel_dial(&peer);

        let (request_tx, request_rx) = mpsc::channel(1024);
        self.driver.insert(peer, request_tx.clone());

        let connection_event_tx = self.connection_event_tx.clone();
        self.driver_set.spawn(async move {
            let ctx = Context::new(connection, request_rx, connection_event_tx, incoming);
            if let Err(e) = driver::start_driver(ctx).await {
                tracing::error!("driver for connection with {peer:?} shutdowned: {e:?}")
            }
            peer
        });

        if let Some(pending_requests) = self.pending_task.remove(&peer) {
            tokio::spawn(async move {
                for req in pending_requests {
                    if request_tx.send(req).await.is_err() {
                        tracing::error!("failed to send pending request to driver");
                    }
                }
            });
        }
    }

    fn enqueue_pending_request(&mut self, peer: NodeIndex, request: DriverRequest) {
        self.pending_task.entry(peer).or_default().push(request);
    }

    fn handle_disconnect(&mut self, peer: NodeIndex) {
        self.driver.remove(&peer);
    }

    // Todo: Return metrics.
    pub async fn start(&mut self) -> Result<()> {
        let endpoint = Endpoint::server(self.server_config.clone(), self.address)?;
        tracing::info!("bound to {:?}", endpoint.local_addr()?);

        self.endpoint = Some(endpoint.clone());

        loop {
            tokio::select! {
                connecting = endpoint.accept() => {
                    match connecting {
                        None => break,
                        Some(connecting) => {
                            tracing::trace!("incoming connection");
                            self.connector.handle_incoming_connection(connecting);
                        }
                    }
                }
                Some(connection_result) = self.connector.advance() => {
                    match connection_result {
                        ConnectionResult::Success { incoming, conn, peer} => {
                            tracing::trace!("new connection with {peer:?}");
                            // The unwrap here is safe because when accepting connections,
                            // we will fail to connect if we cannot obtain the peer's
                            // public key from the TLS session. When dialing, we already
                            // have the peer's public key.
                            self.handle_connection(peer, conn, incoming)
                        }
                        ConnectionResult::Failed { peer, error } => {
                            if peer.is_none() {
                                tracing::warn!("failed to connect to peer: {error:?}");
                            } else {
                                // The unwrap here is safe. See comment above.
                                let peer = peer.unwrap();
                                tracing::warn!("failed to dial peer {:?}: {error:?}", peer);
                                self.connector.remove_pending_dial(&peer);
                            }
                        }
                    }
                }
                broadcast_task = self.broadcast_service.next() => {
                    let Some(broadcast_task) = broadcast_task else {
                        break;
                    };
                    if let Err(e) = self.handle_broadcast_task(broadcast_task) {
                        tracing::error!("failed to handle broadcast task: {e:?}");
                    }
                }
                _broadcast_request = self.stream_service.next() => {

                }
                Some(peer) = self.driver_set.join_next() => {
                    match peer {
                        Ok(pk) => {
                            tracing::trace!("driver finished for connection with {pk:?}");
                            self.handle_disconnect(pk);
                        }
                        Err(e) => {
                            tracing::warn!("unable to clean up failed driver tasks: {e:?}");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub enum ConnectionEvent {
    Broadcast { message: Message },
    Stream { service_scope: ServiceScope },
}

pub struct NodeAddress {
    index: NodeIndex,
    pk: NodePublicKey,
    socket_address: SocketAddr,
}
