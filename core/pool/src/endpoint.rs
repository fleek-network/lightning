use std::cell::OnceCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    Notification,
    ReputationAggregatorInterface,
    ReputationReporterInterface,
    RequestHeader,
    ServiceScope,
    TopologyInterface,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinSet;

use crate::connection::connector::{ConnectionResult, Connector};
use crate::connection::{self, Context, ServiceRequest};
use crate::muxer::{ConnectionInterface, MuxerInterface};
use crate::overlay::{
    BroadcastRequest,
    BroadcastTask,
    Message,
    NetworkOverlay,
    PoolTask,
    SendRequest,
    SendRequestTask,
};
use crate::pool::Request;
use crate::state::{ConnectionInfo, Query, State, TransportConnectionInfo};

type ConnectionId = usize;

const CONN_GRACE_PERIOD: Duration = Duration::from_secs(30);

/// The endpoint for the networking system of the lightning node.
///
/// It provides the following features:
/// - A pool of multiplexed-transport connections.
/// - Broadcasting.
/// - A request/response service
///
/// # Overlay
///
/// The endpoint maintains state for our view of the network overlay
/// in [`NetworkOverlay`]. This object maintains logical connections with
/// a set of peers. These peers are pushed to the [`NetworkOverlay`] during topology
/// updates on every epoch. Broadcast operation are performed only on and for
/// those peers for which we have logical connections to.
///
/// # Pool
///
/// The endpoint has a pool of transport connections which implement
/// the [`MuxerInterface`] trait. When a transport connection is
/// established with a peer, a task is spawned and given ownership
/// of the connection. The connection task will listen for incoming unidirectional
/// streams, for p2p applications such as broadcasting, and for bidirectional
/// streams for request/response applications.
///
/// # Pinning
///
/// To support applications that need to communicate with peers outside of their
/// local network e.g. peers that [`NetworkOverlay`] did not get from topology updates,
/// we allow user tasks to make transport connections to peers outside of their neighborhood.
/// The [`NetworkOverlay`] `pins` these connections in its state so that they may be kept
/// when connections are being dropped during a topology update event.
pub struct Endpoint<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    /// Pool of connections.
    pool: HashMap<NodeIndex, OngoingConnectionHandle>,
    /// Network overlay.
    network_overlay: NetworkOverlay<C>,
    /// Source of network topology.
    topology: c![C::TopologyInterface],
    /// Epoch notifier for triggering polling of topology.
    notifier: Receiver<Notification>,
    /// Report metrics of peers.
    rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    /// Receiver of events from a connection.
    connection_event_rx: Receiver<ConnectionEvent>,
    /// Sender of events from a connection.
    connection_event_tx: Sender<ConnectionEvent>,
    /// Performs dial tasks.
    connector: Connector<M>,
    /// Pending outgoing requests.
    pending_task: HashMap<NodeIndex, Vec<ServiceRequest>>,
    /// Ongoing connection tasks.
    ongoing_connection_tasks: JoinSet<(NodeIndex, ConnectionId)>,
    /// Connections that are redundant.
    // There may be edge cases when two peers connect
    // to each other at the same time. During resolution,
    // instead of closing the extra connection, we
    // hold it in case there is data being
    // transmitted. Nodes will prefer the other connection
    // over this one, so these will eventually close
    // themselves when the idle-timeout triggers.
    // These will need to be garbage collected.
    // Todo: Look into avoiding to maintain two tables.
    redundant_pool: HashMap<NodeIndex, OngoingConnectionHandle>,
    // Before connections are dropped, they will be put in this buffer.
    // After a grace period, the connections will be dropped.
    connection_buffer: Vec<OngoingConnectionHandle>,
    /// Notify when to run clean up task.
    cleanup_notify: Arc<Notify>,
    /// Multiplexed transport.
    muxer: Option<M>,
    /// Request state from the pool.
    query_queue: Receiver<Query>,
    /// Config for the multiplexed transport.
    config: M::Config,
}

impl<C, M> Endpoint<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        topology: c!(C::TopologyInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: Receiver<Notification>,
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        config: M::Config,
        node_public_key: NodePublicKey,
        index: OnceCell<NodeIndex>,
        query_queue: Receiver<Query>,
    ) -> Self {
        let (connection_event_tx, connection_event_rx) = mpsc::channel(1024);

        Self {
            pool: HashMap::new(),
            network_overlay: NetworkOverlay::new(sync_query, node_public_key, index),
            topology,
            notifier,
            rep_reporter,
            connection_event_rx,
            connection_event_tx,
            connector: Connector::new(),
            pending_task: HashMap::new(),
            ongoing_connection_tasks: JoinSet::new(),
            muxer: None,
            config,
            redundant_pool: HashMap::new(),
            connection_buffer: Vec::new(),
            query_queue,
            cleanup_notify: Arc::new(Notify::new()),
        }
    }

    pub fn register_broadcast_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<BroadcastRequest>, Receiver<(NodeIndex, Bytes)>) {
        self.network_overlay
            .register_broadcast_service(service_scope)
    }

    pub fn register_requester_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<SendRequest>, Receiver<(RequestHeader, Request)>) {
        self.network_overlay
            .register_requester_service(service_scope)
    }

    pub fn handle_send_request_task(&mut self, task: SendRequestTask) -> Result<()> {
        let SendRequestTask {
            peer,
            request,
            respond,
            service_scope,
        } = task;

        match self.pool.get_mut(&peer.index) {
            None => {
                let peer_index = peer.index;
                self.connector.enqueue_dial_task(
                    peer,
                    self.muxer
                        .clone()
                        .expect("Endpoint is always initialized on start"),
                )?;

                let request = ServiceRequest::SendRequest {
                    service: service_scope,
                    request,
                    respond,
                };
                self.enqueue_pending_request(peer_index, request);
            },
            Some(handle) => {
                let ongoing_conn_tx = handle.service_request_tx.clone();
                tokio::spawn(async move {
                    let request = ServiceRequest::SendRequest {
                        service: service_scope,
                        request,
                        respond,
                    };
                    if ongoing_conn_tx.send(request).await.is_err() {
                        tracing::error!("failed to send connection loop a send-request task");
                    }
                });
            },
        }
        Ok(())
    }

    pub fn handle_broadcast_task(&mut self, task: BroadcastTask) -> Result<()> {
        match task {
            BroadcastTask::Send {
                service_scope,
                message,
                peers,
            } => {
                // From the all the peers we want to send messages to,
                // we partition into those we are connected to and those
                // that we're not.
                let (connected, not_connected) = {
                    let (connected, not_connected) =
                        peers.into_iter().partition::<Vec<_>, _>(|info| {
                            self.pool.contains_key(&info.node_info.index)
                        });
                    (
                        connected
                            .into_iter()
                            .map(|info| info.node_info.index)
                            .collect::<Vec<_>>(),
                        not_connected,
                    )
                };

                let message = Message {
                    service: service_scope,
                    payload: message.to_vec(),
                };

                tracing::debug!(
                    "received broadcast send request for peers not in the overlay: {not_connected:?}"
                );
                tracing::debug!(
                    "received broadcast send request for connected peers: {connected:?}"
                );

                // We will enqueue a dial task for these peers.
                for info in not_connected {
                    let peer_index = info.node_info.index;
                    if let Err(e) = self.connector.enqueue_dial_task(
                        info.node_info,
                        self.muxer
                            .clone()
                            .expect("Endpoint is always initialized on start"),
                    ) {
                        tracing::error!("failed to enqueue task: {e:?}");
                    }

                    // Enqueue message for later after we connect.
                    self.enqueue_pending_request(
                        peer_index,
                        ServiceRequest::SendMessage(message.clone()),
                    )
                }

                // We already have connections to these peers already
                // so we can just send our message.
                for index in connected {
                    let Some(handle) = self.pool.get(&index) else {
                        tracing::error!("we were told that we had a connection already to peer {index:?}");
                        continue;
                    };

                    let ongoing_conn_tx = handle.service_request_tx.clone();
                    let request = ServiceRequest::SendMessage(message.clone());
                    tokio::spawn(async move {
                        if ongoing_conn_tx.send(request).await.is_err() {
                            tracing::error!(
                                "failed to send broadcast request to connection task for node with index: {index:?}"
                            );
                        }
                    });
                }
            },
            BroadcastTask::Update { keep, drop } => {
                // Move the connections to be dropped into a buffer.
                drop.into_iter().for_each(|index| {
                    if let Some(conn_handle) = self.pool.remove(&index) {
                        self.connection_buffer.push(conn_handle);
                    }
                    // Todo: add unit test for this.
                    if let Some(conn_handle) = self.redundant_pool.remove(&index) {
                        self.connection_buffer.push(conn_handle);
                    }
                });

                // Schedule the internal notification to drop the buffered connections.
                let event_notifier_tx = self.cleanup_notify.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(CONN_GRACE_PERIOD).await;
                    event_notifier_tx.notify_one();
                });

                for info in keep.into_values() {
                    tracing::debug!(
                        "broadcast update: peer with index: {:?}",
                        info.node_info.index
                    );
                    // We do not want to connect to peers we're already connected to
                    // and to peers that should be connecting to us during an update.
                    if self.pool.contains_key(&info.node_info.index) || !info.connect {
                        tracing::debug!(
                            "broadcast update: skip peer with index {:?}",
                            info.node_info.index
                        );
                        continue;
                    }

                    if let Err(e) = self
                        .connector
                        .enqueue_dial_task(info.node_info, self.muxer.clone().unwrap())
                    {
                        tracing::error!("failed to enqueue the dial task: {e:?}");
                    }
                }
            },
        }

        Ok(())
    }

    fn handle_connection(&mut self, connection: M::Connection) {
        let Some(pk) = connection.peer_identity() else {
            tracing::error!("failed to get peer identity from connection");
            return;
        };

        if !self.network_overlay.validate_stake(pk) {
            tracing::info!("peer with pk {pk} failed stake validation: rejecting connection");
            return;
        }

        if let Some(peer_index) = self.network_overlay.pubkey_to_index(pk) {
            self.connector.cancel_dial(&peer_index);

            // We only allow one redundant connection per peer.
            if self.pool.contains_key(&peer_index) && self.redundant_pool.contains_key(&peer_index)
            {
                tracing::warn!("too many redundant connections with peer {peer_index:?}");
                connection.close(0u8, b"close from disconnect");
                return;
            }

            // The connection ID is used for garbage collection.
            // Because a connection could be `incoming` or `outgoing`
            // and because connections can be downgraded if they're redundant,
            // we could inadvertently drop the wrong connection if we only
            // rely on `NodeIndex`.
            let connection_id = connection.connection_id();

            // Report the latency of this peer.
            self.rep_reporter
                .report_ping(peer_index, Some(connection.stats().rtt / 2));

            // Start worker to drive the connection.
            let (request_tx, request_rx) = mpsc::channel(1024);
            let connection_event_tx = self.connection_event_tx.clone();
            let ctx = Context::new(connection, peer_index, request_rx, connection_event_tx);
            self.ongoing_connection_tasks.spawn(async move {
                if let Err(e) = connection::connection_loop(ctx).await {
                    tracing::info!(
                        "task for connection with {peer_index:?} exited with error: {e:?}"
                    );
                }
                (peer_index, connection_id)
            });

            // If connection is not expected for broadcast,
            // we pin the connection.
            let mut should_pin = !self.network_overlay.contains(&peer_index);

            // Handle requests that were waiting for a connection to be established.
            if let Some(pending_requests) = self.pending_task.remove(&peer_index) {
                tracing::debug!(
                    "there are {} pending requests that will be executed now",
                    pending_requests.len()
                );

                for req in pending_requests {
                    // We need to pin the connection if used by requester service.
                    if matches!(req, ServiceRequest::SendRequest { .. }) {
                        should_pin = true;
                    }

                    let request_tx_clone = request_tx.clone();
                    tokio::spawn(async move {
                        if request_tx_clone.send(req).await.is_err() {
                            tracing::error!("failed to send pending request to connection task");
                        }
                    });
                }
            }

            // If connection is not expected for the overlay,
            // we pin the connection.
            // We pin it now because we don't want an update from
            // topology to drop it.
            if should_pin {
                if let Some(info) = self.network_overlay.node_info_from_state(&peer_index) {
                    self.network_overlay.pin_connection(peer_index, info);
                }
            }

            // Save a handle to the connection task to send requests.
            let handle = OngoingConnectionHandle {
                service_request_tx: request_tx,
                connection_id,
            };

            match self.pool.entry(peer_index) {
                Entry::Occupied(_) => {
                    if self.redundant_pool.insert(peer_index, handle).is_some() {
                        tracing::info!(
                            "replacing connection with node with index {peer_index} in redundant pool"
                        );
                    }
                },
                Entry::Vacant(vacant) => {
                    vacant.insert(handle);
                },
            }
        }
    }

    /// Enqueues requests that will be sent after a connection is established with the peer.
    #[inline]
    fn enqueue_pending_request(&mut self, peer: NodeIndex, request: ServiceRequest) {
        self.pending_task.entry(peer).or_default().push(request);
    }

    #[inline]
    fn garbage_collect_closed_connections(&mut self, peer: NodeIndex, connection_id: usize) {
        if let Entry::Occupied(mut entry) = self.pool.entry(peer) {
            // If the connection IDs do not match, another connection was opened or superseded
            // this one so we need to rely on this identifier instead of just the key.
            if entry.get().connection_id == connection_id {
                // Connection ID is unique so this is safe.
                if let Some(handle) = self.redundant_pool.remove(&peer) {
                    entry.insert(handle);
                } else {
                    entry.remove();
                }
            }
        }

        if let Entry::Occupied(entry) = self.redundant_pool.entry(peer) {
            // If the connection IDs do not match, another connection was opened or superseded
            // this one so we need to rely on this identifier instead of just the key.
            if entry.get().connection_id == connection_id {
                entry.remove();
            }
        }

        // Before we attempt to remove any logical pinned connection
        // we must make sure that we don't have any active
        // transport connection.
        if !self.redundant_pool.contains_key(&peer) && !self.pool.contains_key(&peer) {
            self.network_overlay.clean(peer);
        }
    }

    /// Shutdowns workers and clears state.
    pub async fn shutdown(&mut self) {
        self.pool.clear();
        self.redundant_pool.clear();
        self.pending_task.clear();
        self.connector.clear();

        // All connection tasks should finished since we dropped all
        // Senders above.
        while self.ongoing_connection_tasks.join_next().await.is_some() {}

        // We drop the muxer to unbind the address.
        self.muxer
            .take()
            .expect("start method to have been called")
            .close()
            .await;
    }

    #[inline]
    pub fn state(&self, respond: oneshot::Sender<State>) {
        let logical_connections = self.network_overlay.connections();

        let mut actual_connections = self
            .pool
            .iter()
            .map(|(index, handle)| {
                (
                    *index,
                    (
                        self.network_overlay.node_info_from_state(index),
                        handle.service_request_tx.clone(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();

        let redundant_connections = self
            .pool
            .iter()
            .map(|(index, handle)| (*index, handle.service_request_tx.clone()))
            .collect::<HashMap<_, _>>();

        let (broadcast_queue_cap, broadcast_queue_max_cap) =
            self.network_overlay.broadcast_queue_cap();
        let (send_req_queue_cap, send_req_queue_max_cap) =
            self.network_overlay.send_req_queue_cap();

        tokio::spawn(async move {
            let mut connections = HashMap::new();

            // We get information from actual running connections.
            for (index, logical_connection_info) in logical_connections {
                let (respond_tx, respond_rx) = oneshot::channel();
                if let Some((info, queue)) = actual_connections.remove(&index) {
                    // We avoid burdening an already-busy queue.
                    let stats = if queue
                        .try_send(ServiceRequest::Stats {
                            respond: respond_tx,
                        })
                        .is_err()
                    {
                        respond_rx.await.ok()
                    } else {
                        None
                    };

                    let transport_connection = TransportConnectionInfo {
                        redundant: false,
                        stats,
                    };

                    let work_queue_cap = queue.capacity();
                    let work_queue_max_cap = queue.max_capacity();

                    connections.insert(
                        index,
                        ConnectionInfo {
                            from_topology: logical_connection_info.from_topology,
                            pinned: logical_connection_info.pinned,
                            peer: info,
                            work_queue_cap,
                            work_queue_max_cap,
                            actual_connections: vec![transport_connection],
                        },
                    );
                } else {
                    tracing::warn!(
                        "logical connection corresponding to actual connection does not exist"
                    )
                }
            }

            if !actual_connections.is_empty() {
                tracing::warn!("found actual connections that are not in the overlay state")
            }

            // We get information from actual running redundant connections.
            for (index, handle) in redundant_connections {
                let (respond_tx, respond_rx) = oneshot::channel();
                // We avoid burdening an already-busy queue.
                let stats = if handle
                    .try_send(ServiceRequest::Stats {
                        respond: respond_tx,
                    })
                    .is_ok()
                {
                    respond_rx.await.ok()
                } else {
                    None
                };

                if let Some(connection) = connections.get_mut(&index) {
                    let transport_connection = TransportConnectionInfo {
                        redundant: true,
                        stats,
                    };
                    connection.actual_connections.push(transport_connection);
                }
            }

            let _ = respond.send(State {
                broadcast_queue_cap,
                broadcast_queue_max_cap,
                send_req_queue_cap,
                send_req_queue_max_cap,
                connections,
            });
        });
    }

    #[inline]
    pub fn peer_info(&self, index: NodeIndex, respond: oneshot::Sender<Option<ConnectionInfo>>) {
        let info = self.network_overlay.node_info_from_state(&index);
        if let Some(connection_info) = self.network_overlay.get_connection_info(&index) {
            let from_topology = connection_info.from_topology;
            let pinned = connection_info.pinned;

            if let Some(handle) = self.pool.get(&index) {
                let (respond_tx, respond_rx) = oneshot::channel();
                let work_queue_cap = handle.service_request_tx.capacity();
                let work_queue_max_cap = handle.service_request_tx.max_capacity();

                if handle
                    .service_request_tx
                    .try_send(ServiceRequest::Stats {
                        respond: respond_tx,
                    })
                    .is_ok()
                {
                    tokio::spawn(async move {
                        let stats = respond_rx.await.ok();
                        let transport_connection = TransportConnectionInfo {
                            redundant: false,
                            stats,
                        };
                        let _ = respond.send(Some(ConnectionInfo {
                            from_topology,
                            pinned,
                            peer: info,
                            work_queue_cap,
                            work_queue_max_cap,
                            actual_connections: vec![transport_connection],
                        }));
                    });
                } else {
                    let _ = respond.send(Some(ConnectionInfo {
                        from_topology,
                        pinned,
                        peer: info,
                        work_queue_cap,
                        work_queue_max_cap,
                        actual_connections: vec![],
                    }));
                }
                // Note: we ignore redundant connection because they should not get used much
                // and should not happen often unless there's some malicious nodes.
            } else {
                let _ = respond.send(None);
            }
        }
    }

    #[inline]
    pub fn handle_query(&self, query: Query) {
        match query {
            Query::State { respond } => self.state(respond),
            Query::Peer { index, respond } => self.peer_info(index, respond),
        }
    }

    // Todo: Return metrics.
    pub async fn start(&mut self, shutdown: Arc<Notify>) -> Result<()> {
        let muxer = M::init(self.config.clone())?;
        self.muxer = Some(muxer.clone());

        let new_connections = self
            .topology
            .suggest_connections()
            .iter()
            .flatten()
            .filter_map(|pk| self.network_overlay.pubkey_to_index(*pk))
            .filter(|index| *index != self.network_overlay.get_index())
            .collect::<HashSet<_>>();
        let broadcast_task = self.network_overlay.update_connections(new_connections);
        if let Err(e) = self.handle_broadcast_task(broadcast_task) {
            tracing::error!("failed to handle broadcast task: {e:?}");
        }

        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    break;
                }
                connecting = muxer.accept() => {
                    match connecting {
                        None => break,
                        Some(connecting) => {
                            self.connector.handle_incoming_connection(connecting);
                        }
                    }
                }
                Some(event) = self.connection_event_rx.recv() => {
                    match event {
                        ConnectionEvent::Broadcast { peer, message } => {
                            self.network_overlay.handle_broadcast_message(peer, message);
                        },
                        ConnectionEvent::IncomingRequest { peer, service_scope, request } => {
                            self.network_overlay.handle_incoming_request(
                                peer,
                                service_scope,
                                request
                            );
                        }
                    }
                }
                Some(connection_result) = self.connector.advance() => {
                    match connection_result {
                        ConnectionResult::Success { conn, .. } => {
                            self.handle_connection(conn);
                        }
                        ConnectionResult::Failed { peer, error } => {
                            if peer.is_none() {
                                tracing::warn!("failed to connect to peer: {error:?}");
                            } else {
                                let peer = peer.unwrap();
                                tracing::warn!("failed to dial peer {:?}: {error:?}", peer);
                                self.connector.remove_pending_dial(&peer);
                            }
                        }
                    }
                }
                task = self.network_overlay.next() => {
                    let Some(task) = task else {
                        break;
                    };
                    match task {
                        PoolTask::Broadcast(task) => {
                            if let Err(e) = self.handle_broadcast_task(task) {
                                tracing::error!("failed to handle broadcast task: {e:?}");
                            }
                        }
                        PoolTask::SendRequest(task) => {
                            if let Err(e) = self.handle_send_request_task(task) {
                                tracing::error!("failed to handle send-request task: {e:?}");
                            }
                        }
                    }
                }
                epoch_event = self.notifier.recv() => {
                    match epoch_event {
                        Some(Notification::NewEpoch) => {
                            let new_connections = self
                                .topology
                                .suggest_connections()
                                .iter()
                                .flatten()
                                .filter_map(|pk| self.network_overlay.pubkey_to_index(*pk))
                                .filter(|index| *index != self.network_overlay.get_index())
                                .collect::<HashSet<_>>();
                            let broadcast_task =  self
                                .network_overlay
                                .update_connections(new_connections);
                            if let Err(e) = self.handle_broadcast_task(broadcast_task) {
                                tracing::error!("failed to handle broadcast task after an update: {e:?}");
                            }
                        }
                        Some(Notification::BeforeEpochChange) => {
                            unreachable!("we're only registered for new epoch events")
                        }
                        None => {
                            tracing::info!("notifier was dropped");
                            break;
                        }
                    }
                }
                _ = self.cleanup_notify.notified() => {
                    self.connection_buffer.clear();
                }
                // It's safe to match on Some(_) here because `poll_next`
                // will return None when there are no ongoing connections
                // and thus, it should be disabled to allow other branches
                // to process new connections.
                Some(peer) = self.ongoing_connection_tasks.join_next() => {
                    match peer {
                        Ok((index, id)) => {
                            tracing::trace!("task for connection={id:?} with node={index:?} ended");
                            self.garbage_collect_closed_connections(index, id);
                        }
                        Err(e) => {
                            tracing::warn!("unable to clean up failed connection tasks: {e:?}");
                        }
                    }
                }
                next_query = self.query_queue.recv() => {
                    if let Some(query) = next_query {
                        self.handle_query(query);
                    };
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
    IncomingRequest {
        peer: NodeIndex,
        service_scope: ServiceScope,
        request: (RequestHeader, Request),
    },
}

pub struct OngoingConnectionHandle {
    service_request_tx: Sender<ServiceRequest>,
    connection_id: usize,
}
