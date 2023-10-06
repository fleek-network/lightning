use std::cell::OnceCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

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
    ServiceScope,
    TopologyInterface,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinSet;

use crate::connection::connector::{ConnectionResult, Connector};
use crate::connection::driver::{self, Context, DriverRequest};
use crate::muxer::{Channel, ConnectionInterface, MuxerInterface};
use crate::overlay::{
    BroadcastRequest,
    BroadcastTask,
    Message,
    NetworkOverlay,
    PoolTask,
    StreamRequest,
    StreamTask,
};

type ConnectionId = usize;

/// The endpoint for the networking system of the lightning node.
///
/// It provides the following features:
/// - A pool of multiplexed-transport connections.
/// - Broadcasting.
/// - [`Channels`] for direct task to task communication over the network.
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
/// established with a peer, a driver tasks is spawned and given ownership
/// of the connection. The driver task will listen for incoming unidirectional
/// streams, for p2p applications such as broadcasting, and for bidirectional
/// streams for request/response applications. Bidirectional streams are used
/// for implementing [`Channel`]s.
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
    pool: HashMap<NodeIndex, DriverHandle>,
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
    pending_task: HashMap<NodeIndex, Vec<DriverRequest>>,
    /// Ongoing drivers.
    driver_set: JoinSet<(NodeIndex, ConnectionId)>,
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
    redundant_pool: HashMap<NodeIndex, DriverHandle>,
    /// Multiplexed transport.
    muxer: Option<M>,
    /// Config for the multiplexed transport.
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
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        config: M::Config,
        node_public_key: NodePublicKey,
        index: OnceCell<NodeIndex>,
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
            driver_set: JoinSet::new(),
            muxer: None,
            config,
            redundant_pool: HashMap::new(),
        }
    }

    pub fn register_broadcast_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<BroadcastRequest>, Receiver<(NodeIndex, Bytes)>) {
        self.network_overlay
            .register_broadcast_service(service_scope)
    }

    pub fn register_stream_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<StreamRequest>, Receiver<Channel>) {
        self.network_overlay.register_stream_service(service_scope)
    }

    pub fn handle_stream_request(&mut self, task: StreamTask) -> Result<()> {
        let StreamTask {
            peer,
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

                // Todo: possible optimization would be to give more priority to
                // NewStream requests.
                let request = DriverRequest::NewStream {
                    service: service_scope,
                    respond,
                };
                self.enqueue_pending_request(peer_index, request);
            },
            Some(handle) => {
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
                let (connected, not_connected) = match peers {
                    Some(peers) => {
                        let (connected, not_connected) =
                            peers.into_iter().partition::<Vec<_>, _>(|info| {
                                self.pool.contains_key(&info.address.index)
                            });
                        (
                            connected
                                .into_iter()
                                .map(|info| info.address.index)
                                .collect::<Vec<_>>(),
                            not_connected,
                        )
                    },
                    None => (self.pool.keys().copied().collect::<Vec<_>>(), vec![]),
                };

                let message = Message {
                    service: service_scope,
                    payload: message.to_vec(),
                };

                // We will enqueue a dial task for these peers.
                for info in not_connected {
                    tracing::debug!("received send request for peers not in the overlay");

                    let peer_index = info.address.index;
                    if let Err(e) = self.connector.enqueue_dial_task(
                        info.address,
                        self.muxer
                            .clone()
                            .expect("Endpoint is always initialized on start"),
                    ) {
                        tracing::error!("failed to enqueue task: {e:?}");
                    }

                    // Enqueue message for later after we connect.
                    self.enqueue_pending_request(
                        peer_index,
                        DriverRequest::Message(message.clone()),
                    )
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
                // Keep ongoing connections.
                self.pool.retain(|index, _| peers.contains_key(index));

                for info in peers.into_values() {
                    // We do not want to connect to peers we're already connected to
                    // and to peers that should be connecting to us during an update.
                    if self.pool.contains_key(&info.address.index) || !info.connect {
                        continue;
                    }

                    if let Err(e) = self
                        .connector
                        .enqueue_dial_task(info.address, self.muxer.clone().unwrap())
                    {
                        tracing::error!("failed to enqueue the dial task: {e:?}");
                    }
                }
            },
        }

        Ok(())
    }

    fn handle_connection(&mut self, connection: M::Connection) {
        if let Some(peer_index) = self.network_overlay.index_from_connection::<M>(&connection) {
            self.connector.cancel_dial(&peer_index);

            // Redundant connections arent dropped immediately. Please see comment in `Endpoint`.
            let is_redundant = self.network_overlay.is_redundant(&peer_index);

            // We only allow one redundant connection per peer.
            if is_redundant && self.redundant_pool.contains_key(&peer_index) {
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
            // Todo: Remove this `if` when we switch to using NodeIndex.
            if let Some(pk) = self.network_overlay.index_to_pubkey(peer_index) {
                self.rep_reporter
                    .report_latency(&pk, connection.metrics().rtt);
            }

            // Start worker to drive the connection.
            let (request_tx, request_rx) = mpsc::channel(1024);
            let connection_event_tx = self.connection_event_tx.clone();
            let ctx = Context::new(connection, peer_index, request_rx, connection_event_tx);
            self.driver_set.spawn(async move {
                if let Err(e) = driver::start_driver(ctx).await {
                    tracing::info!(
                        "driver for connection with {peer_index:?} exited with error: {e:?}"
                    );
                }
                (peer_index, connection_id)
            });

            // If connection is not expected for broadcast,
            // we pin the connection.
            let mut should_pin = !self.network_overlay.contains(&peer_index);

            // Handle requests that were waiting for a connection to be established.
            if let Some(pending_requests) = self.pending_task.remove(&peer_index) {
                for req in pending_requests {
                    // We need to pin the connection if used by stream service.
                    should_pin = matches!(req, DriverRequest::NewStream { .. });

                    let request_tx_clone = request_tx.clone();
                    tokio::spawn(async move {
                        if request_tx_clone.send(req).await.is_err() {
                            tracing::error!("failed to send pending request to driver");
                        }
                    });
                }
            }

            // If connection is not expected for the overlay,
            // we pin the connection.
            // We pin it now because we don't want an update from
            // topology to drop it.
            if should_pin {
                if let Some(address) = self.network_overlay.node_address_from_state(&peer_index) {
                    self.network_overlay.pin_connection(peer_index, address);
                }
            }

            // Save a handle to the driver to send requests.
            let handle = DriverHandle {
                tx: request_tx,
                connection_id,
            };

            match self.pool.entry(peer_index) {
                Entry::Occupied(mut entry) => {
                    if is_redundant {
                        self.redundant_pool.insert(peer_index, handle);
                    } else {
                        // The old connection could be in the middle of a write,
                        // so we give it a chance to finish.
                        let old = entry.insert(handle);
                        self.redundant_pool.insert(peer_index, old);
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
    fn enqueue_pending_request(&mut self, peer: NodeIndex, request: DriverRequest) {
        self.pending_task.entry(peer).or_default().push(request);
    }

    #[inline]
    fn garbage_collect_closed_connections(&mut self, peer: NodeIndex, connection_id: usize) {
        if let Entry::Occupied(entry) = self.pool.entry(peer) {
            // If the connection IDs do not match, another connection was opened or superseded
            // this one so we need to rely on this identifier instead of just the key.
            if entry.get().connection_id == connection_id {
                entry.remove();
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

        // All driver tasks should finished since we dropped all
        // Senders above.
        while self.driver_set.join_next().await.is_some() {}

        // We drop the muxer to unbind the address.
        self.muxer.take();
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
                        ConnectionEvent::Stream { peer, service_scope, stream } => {
                            self.network_overlay.handle_incoming_stream(peer, service_scope, stream)
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
                        PoolTask::Stream(task) => {
                            if let Err(e) = self.handle_stream_request(task) {
                                tracing::error!("failed to handle stream request: {e:?}");
                            }
                        }
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
                        Notification::BeforeEpochChange => {
                            unreachable!("we're only registered for new epoch events")
                        }
                    }
                }
                Some(peer) = self.driver_set.join_next() => {
                    match peer {
                        Ok((index, id)) => {
                            tracing::trace!("driver for connection={id:?} with node={index:?} ended");
                            self.garbage_collect_closed_connections(index, id);
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
#[derive(Clone)]
pub struct NodeAddress {
    pub index: NodeIndex,
    pub pk: NodePublicKey,
    pub socket_address: SocketAddr,
}

pub struct DriverHandle {
    tx: Sender<DriverRequest>,
    connection_id: usize,
}
