use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    Notification,
    ServiceScope,
    SyncQueryRunnerInterface,
    TopologyInterface,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinSet;

use crate::connection::connector::{ConnectionResult, Connector};
use crate::connection::driver::{self, Context, DriverRequest};
use crate::muxer::{Channel, MuxerInterface};
use crate::service::broadcast::{BroadcastRequest, BroadcastService, BroadcastTask, Message};
use crate::service::stream::{StreamRequest, StreamService};

/// Endpoint for pool
pub struct Endpoint<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    /// Pool of connections.
    pool: HashMap<NodeIndex, DriverHandle>,
    /// Used for getting peer information from state.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Network broadcast service.
    broadcast_service: BroadcastService<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>,
    /// Receive requests for a multiplexed stream.
    stream_service: StreamService,
    /// Source of network topology.
    topology: c![C::TopologyInterface],
    /// Epoch notifier for triggering polling of topology.
    notifier: Receiver<Notification>,
    /// Receiver of events from a connection.
    connection_event_rx: Receiver<ConnectionEvent>,
    /// Sender of events from a connection.
    connection_event_tx: Sender<ConnectionEvent>,
    /// Performs dial tasks.
    connector: Connector<C, M>,
    /// Pending outgoing requests.
    pending_task: HashMap<NodeIndex, Vec<DriverRequest>>,
    /// Ongoing drivers.
    driver_set: JoinSet<NodeIndex>,
    // Todo: Make an interface to abstract Endpoint.
    // This will allow us to use other protocols besides QUIC.
    /// QUIC Endpoint.
    muxer: Option<M>,
    config: M::Config,
}

impl<C, M> Endpoint<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    pub fn new(
        topology: c!(C::TopologyInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: Receiver<Notification>,
        config: M::Config,
    ) -> Self {
        let (connection_event_tx, connection_event_rx) = mpsc::channel(1024);

        Self {
            pool: HashMap::new(),
            sync_query: sync_query.clone(),
            broadcast_service: BroadcastService::new(),
            stream_service: StreamService::new(),
            topology,
            notifier,
            connection_event_rx,
            connection_event_tx,
            connector: Connector::new(sync_query),
            pending_task: HashMap::new(),
            driver_set: JoinSet::new(),
            muxer: None,
            config,
        }
    }

    pub fn register_broadcast_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (
        Sender<BroadcastRequest<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>>,
        Receiver<(NodeIndex, Bytes)>,
    ) {
        self.broadcast_service.register(service_scope)
    }

    pub fn register_stream_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<StreamRequest>, Receiver<Channel>) {
        self.stream_service.register(service_scope)
    }

    pub fn handle_stream_request(&mut self, request: StreamRequest) -> anyhow::Result<()> {
        let StreamRequest {
            peer,
            respond,
            service_scope,
        } = request;
        match self.pool.get_mut(&peer) {
            None => {
                let pk = self
                    .sync_query
                    .index_to_pubkey(peer)
                    .ok_or(anyhow::anyhow!(""))?;
                let info = self
                    .sync_query
                    .get_node_info(&pk)
                    .ok_or(anyhow::anyhow!(""))?;
                let address = NodeAddress {
                    index: peer,
                    pk,
                    socket_address: SocketAddr::from((info.domain, info.ports.pool)),
                };

                self.connector.enqueue_dial_task(
                    address,
                    self.muxer
                        .clone()
                        .expect("Endpoint is always initialized on start"),
                )?;

                // Todo: possible optimization would be to give more priority to
                // NewStream requests.
                let request = DriverRequest::NewStream {
                    service: service_scope,
                    respond,
                };
                self.enqueue_pending_request(peer, request);
            },
            Some(handle) => {
                // We pin this now because we dont want to drop this
                // on a topology change. See `DriverHandle`.
                handle.pinned = true;
                let driver_tx = handle.tx.clone();
                tokio::spawn(async move {
                    let request = DriverRequest::NewStream {
                        service: service_scope,
                        respond,
                    };
                    if driver_tx.send(request).await.is_err() {
                        tracing::error!("failed to send driver stream request");
                    }
                });
            },
        }
        Ok(())
    }

    pub fn handle_broadcast_task(&mut self, task: BroadcastTask) -> anyhow::Result<()> {
        match task {
            BroadcastTask::Send {
                service_scope,
                message,
                peers,
            } => {
                // From the all the peers we want to send messages to,
                // we partition into those we are connected to and those
                // that we're not.
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

                // We will enqueue a dial task for these peers.
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
                        self.muxer
                            .clone()
                            .expect("Endpoint is always initialized on start"),
                    )?;
                    // Enqueue message for later after we connect.
                    self.enqueue_pending_request(index, DriverRequest::Message(message.clone()))
                }

                // We already have connections to these peers already
                // so we can just send our message.
                for index in connected {
                    let Some(handle) = self.pool.get(&index) else {
                        tracing::error!("we were told that we had a connection already to peer {index:?}");
                        continue;
                    };

                    let driver_tx = handle.tx.clone();
                    let request = DriverRequest::Message(message.clone());
                    tokio::spawn(async move {
                        if driver_tx.send(request).await.is_err() {
                            tracing::error!("failed to send driver an outgoing broadcast request");
                        }
                    });
                }
            },
            BroadcastTask::Update { peers } => {
                for index in peers.iter() {
                    let Some(handle) = self.pool.get(index) else {
                        continue;
                    };

                    // We don't want to disconnect pinned connections.
                    if handle.pinned {
                        continue;
                    }

                    let driver_tx = handle.tx.clone();
                    tokio::spawn(async move {
                        if driver_tx.send(DriverRequest::Disconnect).await.is_err() {
                            tracing::error!("failed to send driver a disconnect request");
                        }
                    });
                }
                self.pool.retain(|index, _| !peers.contains(index));
            },
        }

        Ok(())
    }

    fn handle_connection(&mut self, peer: NodeIndex, connection: M::Connection, incoming: bool) {
        self.connector.cancel_dial(&peer);

        // Start worker to drive the connection.
        let (request_tx, request_rx) = mpsc::channel(1024);
        let connection_event_tx = self.connection_event_tx.clone();
        self.driver_set.spawn(async move {
            let ctx = Context::new(connection, peer, request_rx, connection_event_tx, incoming);
            if let Err(e) = driver::start_driver(ctx).await {
                tracing::error!("driver for connection with {peer:?} shutdowned: {e:?}")
            }
            peer
        });

        // If connection is not expected for broadcast,
        // we pin the connection.
        let mut pin = !incoming && !self.broadcast_service.contains(&peer);

        // Handle requests that were waiting for a connection to be established.
        if let Some(pending_requests) = self.pending_task.remove(&peer) {
            for req in pending_requests {
                // We need to pin the connection if used by stream service.
                pin = matches!(req, DriverRequest::NewStream { .. });

                let request_tx_clone = request_tx.clone();
                tokio::spawn(async move {
                    if request_tx_clone.send(req).await.is_err() {
                        tracing::error!("failed to send pending request to driver");
                    }
                });
            }
        }

        // Save a handle to the driver to send requests.
        let handle = DriverHandle {
            pinned: pin,
            tx: request_tx.clone(),
        };
        self.pool.insert(peer, handle);
    }

    /// Enqueues requests that will be sent after a connection is established with the peer.
    #[inline]
    fn enqueue_pending_request(&mut self, peer: NodeIndex, request: DriverRequest) {
        self.pending_task.entry(peer).or_default().push(request);
    }

    #[inline]
    fn handle_disconnect(&mut self, peer: NodeIndex) {
        self.pool.remove(&peer);
    }

    #[inline]
    fn pin_connection(&mut self, peer: NodeIndex) {
        self.pool
            .entry(peer)
            .and_modify(|handle| handle.pinned = true);
    }

    /// Shutdowns workers and clears state.
    pub async fn shutdown(&mut self) {
        for handle in self.pool.values() {
            let _ = handle.tx.send(DriverRequest::Disconnect).await;
        }

        self.pool.clear();
        self.pending_task.clear();
        self.connector.clear();

        while let Some(_) = self.driver_set.join_next().await {}

        // We drop the muxer to unbind the address.
        self.muxer.take();
    }

    // Todo: Return metrics.
    pub async fn start(&mut self, shutdown: Arc<Notify>) -> anyhow::Result<()> {
        let muxer = M::init(self.config.clone())?;
        self.muxer = Some(muxer.clone());

        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    break;
                }
                connecting = muxer.accept() => {
                    match connecting {
                        None => break,
                        Some(connecting) => {
                            tracing::trace!("incoming connection");
                            self.connector.handle_incoming_connection(connecting);
                        }
                    }
                }
                Some(event) = self.connection_event_rx.recv() => {
                    match event {
                        ConnectionEvent::Broadcast { peer, message } => {
                            self.broadcast_service.handle_broadcast_message(peer, message);
                        },
                        ConnectionEvent::Stream { peer, service_scope, stream } => {
                            tracing::trace!("incoming stream from peer {peer:?}");
                            // We want to make sure that this request passes some basic checks
                            // before we commit to pinning the connection.
                            if self.stream_service.handle_incoming_stream(service_scope, stream) {
                                self.pin_connection(peer);
                            }

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
                stream_request = self.stream_service.next() => {
                    let Some(stream_request) = stream_request else {
                        break;
                    };
                    if let Err(e) = self.handle_stream_request(stream_request) {
                        tracing::error!("failed to handle stream request: {e:?}");
                    }
                }
                Some(epoch_event) = self.notifier.recv() => {
                    match epoch_event {
                        Notification::NewEpoch => {
                            let new_connections = self
                                .topology
                                .suggest_connections()
                                .iter()
                                .flatten()
                                .filter_map(|pk| self.sync_query.pubkey_to_index(*pk))
                                .collect::<HashSet<_>>();
                            let broadcast_task =  self
                                .broadcast_service
                                .update_connections(new_connections);
                            if let Err(e) = self.handle_broadcast_task(broadcast_task) {
                                tracing::error!("failed to handle broadcast task: {e:?}");
                            }
                        }
                        Notification::BeforeEpochChange => {
                            unreachable!("we're only registered for new epoch events")
                        }
                    }
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

/// An event associated to a connection.
pub enum ConnectionEvent {
    Broadcast {
        peer: NodeIndex,
        message: Message,
    },
    Stream {
        peer: NodeIndex,
        service_scope: ServiceScope,
        stream: Channel,
    },
}

/// Address of a peer node.
pub struct NodeAddress {
    pub index: NodeIndex,
    pub pk: NodePublicKey,
    pub socket_address: SocketAddr,
}

pub struct DriverHandle {
    tx: Sender<DriverRequest>,
    // Pinned connections are those connections used by the
    // stream service. We may not want to disconnect these
    // during a topology event because they might be in used by
    // the stream users.
    //
    // Todo
    // Research: Will incoming topology-related connections
    // always be pinned because we don't know them?
    pinned: bool,
}
