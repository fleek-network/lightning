use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io;
use std::time::Duration;

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, ServiceScope, SyncQueryRunnerInterface};
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

use crate::connection;
use crate::connection::{Context, ServiceRequest};
use crate::endpoint::OngoingConnectionHandle;
use crate::event::{Event, PoolTask};
use crate::muxer::{ConnectionInterface, MuxerInterface};
use crate::overlay::{ConnectionInfo, Message};
use crate::provider::Response;
use crate::state::{NodeInfo, Stats};

type ConnectionId = usize;

const CONN_GRACE_PERIOD: Duration = Duration::from_secs(30);

pub struct Endpoint<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    /// Pool of connections.
    pool: HashMap<NodeIndex, OngoingConnectionHandle>,
    /// Queue of incoming tasks.
    task_queue: Receiver<PoolTask>,
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
    /// Sender for events.
    event_queue: Sender<Event>,
    /// Query runner to validate incoming connections.
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    /// Multiplexed transport.
    muxer: Option<M>,
    /// Config for the multiplexed transport.
    config: M::Config,
    shutdown: CancellationToken,
}

impl<C, M> Endpoint<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        task_queue: Receiver<PoolTask>,
        event_queue: Sender<Event>,
        config: M::Config,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            pool: HashMap::new(),
            task_queue,
            pending_task: HashMap::new(),
            ongoing_connection_tasks: JoinSet::new(),
            redundant_pool: HashMap::new(),
            connection_buffer: Vec::new(),
            event_queue,
            query_runner,
            muxer: None,
            config,
            shutdown,
        }
    }

    fn enqueue_dial_task(&mut self, info: NodeInfo, muxer: M) -> anyhow::Result<()> {
        todo!()
    }

    /// Enqueues requests that will be sent after a connection is established with the peer.
    #[inline]
    fn enqueue_pending_request(&mut self, peer: NodeIndex, request: ServiceRequest) {
        self.pending_task.entry(peer).or_default().push(request);
    }

    fn handle_outgoing_request(
        &mut self,
        dst: NodeInfo,
        service: ServiceScope,
        request: Bytes,
        respond: oneshot::Sender<io::Result<Response>>,
    ) -> anyhow::Result<()> {
        match self.pool.get_mut(&dst.index) {
            None => {
                let peer_index = dst.index;
                self.enqueue_dial_task(
                    dst,
                    self.muxer
                        .clone()
                        .expect("Endpoint is always initialized on start"),
                )?;

                let request = ServiceRequest::SendRequest {
                    service,
                    request,
                    respond,
                };
                self.enqueue_pending_request(peer_index, request);
            },
            Some(handle) => {
                let ongoing_conn_tx = handle.service_request_tx.clone();
                ongoing_conn_tx.try_send(ServiceRequest::SendRequest {
                    service,
                    request,
                    respond,
                })?;
            },
        }
        Ok(())
    }

    fn handle_outgoing_message(
        &mut self,
        dst: Vec<ConnectionInfo>,
        message: Message,
    ) -> anyhow::Result<()> {
        // From the all the peers we want to send messages to,
        // we partition into those we are connected to and those
        // that we're not.
        let (connected, not_connected) = {
            let (connected, not_connected) = dst
                .into_iter()
                .partition::<Vec<_>, _>(|info| self.pool.contains_key(&info.node_info.index));
            (
                connected
                    .into_iter()
                    .map(|info| info.node_info.index)
                    .collect::<Vec<_>>(),
                not_connected,
            )
        };

        tracing::debug!(
            "received broadcast send request for peers not in the overlay: {not_connected:?}"
        );
        tracing::debug!("received broadcast send request for connected peers: {connected:?}");

        // We will enqueue a dial task for these peers.
        for info in not_connected {
            let peer_index = info.node_info.index;
            if let Err(e) = self.enqueue_dial_task(
                info.node_info,
                self.muxer
                    .clone()
                    .expect("Endpoint is always initialized on start"),
            ) {
                tracing::error!("failed to enqueue task: {e:?}");
            }

            // Enqueue message for later after we connect.
            self.enqueue_pending_request(peer_index, ServiceRequest::SendMessage(message.clone()))
        }

        // We already have connections to these peers already
        // so we can just send our message.
        for index in connected {
            let Some(handle) = self.pool.get(&index) else {
                tracing::error!("we were told that we had a connection already to peer {index:?}");
                continue;
            };

            let ongoing_conn_tx = handle.service_request_tx.clone();
            if ongoing_conn_tx
                .try_send(ServiceRequest::SendMessage(message.clone()))
                .is_err()
            {
                tracing::error!(
                    "failed to send broadcast request to connection task for node with index: {index:?}"
                );
            }
        }

        Ok(())
    }

    fn handle_update(
        &mut self,
        keep: HashMap<NodeIndex, ConnectionInfo>,
        drop: Vec<NodeIndex>,
    ) -> anyhow::Result<()> {
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

        // Todo: implement this.
        // Schedule the internal notification to drop the buffered connections.
        // let event_notifier_tx = self.cleanup_notify.clone();
        // tokio::spawn(async move {
        //     tokio::time::sleep(CONN_GRACE_PERIOD).await;
        //     event_notifier_tx.notify_one();
        // });

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

            if let Err(e) = self.enqueue_dial_task(info.node_info, self.muxer.clone().unwrap()) {
                tracing::error!("failed to enqueue the dial task: {e:?}");
            }
        }

        Ok(())
    }

    /// Returns true if the peer has staked the required amount
    /// to be a valid node in the network, and false otherwise.
    #[inline]
    fn validate_stake(&self, peer: NodePublicKey) -> bool {
        match self.query_runner.pubkey_to_index(&peer) {
            None => false,
            Some(ref node_idx) => {
                HpUfixed::from(self.query_runner.get_staking_amount())
                    <= self
                        .query_runner
                        .get_node_info::<HpUfixed<18>>(node_idx, |n| n.stake.staked)
                        .unwrap_or(HpUfixed::<18>::zero())
            },
        }
    }

    fn cancel_dial(&mut self, dst: &NodeIndex) {
        todo!()
    }

    fn handle_incoming_connection(&mut self, connection: M::Connection) {
        let Some(pk) = connection.peer_identity() else {
            tracing::error!("failed to get peer identity from connection");
            return;
        };

        if !self.validate_stake(pk) {
            tracing::info!("peer with pk {pk} failed stake validation: rejecting connection");
            return;
        }

        if let Some(peer_index) = self.query_runner.pubkey_to_index(&pk) {
            self.cancel_dial(&peer_index);

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

            // Start worker to drive the connection.
            let (request_tx, request_rx) = mpsc::channel(1024);
            let connection_event_tx = self.event_queue.clone();
            let ctx = Context::new(connection, peer_index, request_rx, connection_event_tx);
            self.ongoing_connection_tasks.spawn(async move {
                if let Err(e) = connection::connection_loop(ctx).await {
                    tracing::info!(
                        "task for connection with {peer_index:?} exited with error: {e:?}"
                    );
                }
                (peer_index, connection_id)
            });

            // We need to pin the connection if used by requester service.
            let mut service_request_sent = false;

            // Handle requests that were waiting for a connection to be established.
            if let Some(pending_requests) = self.pending_task.remove(&peer_index) {
                tracing::debug!(
                    "there are {} pending requests that will be executed now",
                    pending_requests.len()
                );

                for req in pending_requests {
                    if matches!(req, ServiceRequest::SendRequest { .. }) {
                        service_request_sent = true;
                    }

                    let request_tx_clone = request_tx.clone();
                    if request_tx_clone.try_send(req).is_err() {
                        tracing::error!("failed to send pending request to connection task");
                    }
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

            if self
                .event_queue
                .try_send(Event::NewConnection {
                    remote: peer_index,
                    service_request_sent,
                })
                .is_err()
            {
                tracing::error!("failed to send new connection event");
            }
        }
    }

    fn handle_stats_request(&mut self, respond: oneshot::Sender<Stats>) -> anyhow::Result<()> {
        todo!()
    }

    fn handle_task(&mut self, task: PoolTask) -> anyhow::Result<()> {
        match task {
            PoolTask::SendMessage { peers, message } => {
                let _ = self.handle_outgoing_message(peers, message);
            },
            PoolTask::SendRequest {
                dst,
                service,
                request,
                respond,
            } => {
                let _ = self.handle_outgoing_request(dst, service, request, respond);
            },
            PoolTask::Update { keep, drop } => {
                let _ = self.handle_update(keep, drop);
            },
            PoolTask::Stats { respond } => {
                let _ = self.handle_stats_request(respond);
            },
        }

        Ok(())
    }

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
            let _ = self.event_queue.try_send(Event::ConnectionEnded(peer));
        }
    }

    /// Shutdowns workers and clears state.
    pub async fn shutdown(&mut self) {
        self.pool.clear();
        self.redundant_pool.clear();
        self.pending_task.clear();

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

    pub fn spawn(mut self) -> JoinHandle<Self> {
        tokio::spawn(async move {
            let _ = self.run().await;
            self
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let muxer = M::init(self.config.clone())?;
        self.muxer = Some(muxer.clone());

        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    break;
                }
                next = muxer.accept() => {
                    if let Some(connection) = next {
                        match connection {
                            Ok(connection) => {
                                let _ = self.handle_incoming_connection(connection);
                            }
                            Err(e) => {
                                tracing::info!("failed to connect: {e:?}");
                            }
                        }
                    }
                }
                next = self.task_queue.recv() => {
                    if let Some(task) = next {
                       let _ = self.handle_task(task);
                    }
                }
            }
        }

        Ok(())
    }
}
