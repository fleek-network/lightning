use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io;
use std::time::Duration;

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use hp_fixed::unsigned::HpUfixed;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, ServiceScope, SyncQueryRunnerInterface};
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::connection;
use crate::connection::{Context, ServiceRequest};
use crate::event::{ConnectionInfo, Event, Message, PoolTask};
use crate::muxer::{ConnectionInterface, MuxerInterface};
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
    /// Queue of dial tasks.
    pending_dial: HashMap<NodeIndex, CancellationToken>,
    /// Pending outgoing requests.
    pending_task: HashMap<NodeIndex, Vec<ServiceRequest>>,
    /// Pool for asynchronous tasks.
    task_pool: FuturesUnordered<JoinHandle<AsyncTaskResult<M::Connection>>>,
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
            pending_dial: HashMap::new(),
            redundant_pool: HashMap::new(),
            connection_buffer: Vec::new(),
            task_pool: FuturesUnordered::new(),
            event_queue,
            query_runner,
            muxer: None,
            config,
            shutdown,
        }
    }

    fn enqueue_dial_task(&mut self, info: NodeInfo, muxer: M) -> anyhow::Result<()> {
        if let Entry::Vacant(entry) = self.pending_dial.entry(info.index) {
            let cancel = CancellationToken::new();
            entry.insert(cancel.clone());

            let handle = tokio::spawn(async move {
                let index = info.index;
                let connection = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return AsyncTaskResult::ConnectionFailed {
                        remote: Some(index),
                        error: anyhow::anyhow!("dial was cancelled")
                    },
                    connection = muxer.connect(info, "lightning-node") => connection,
                };
                match connection {
                    Ok(conn) => AsyncTaskResult::ConnectionSuccess {
                        incoming: false,
                        conn,
                    },
                    Err(e) => AsyncTaskResult::ConnectionFailed {
                        remote: Some(index),
                        error: e.into(),
                    },
                }
            });

            self.task_pool.push(handle);
        }

        Ok(())
    }

    #[inline]
    pub fn remove_pending_dial(&mut self, peer: &NodeIndex) {
        self.pending_dial.remove(peer);
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

    #[inline]
    pub fn schedule_timeout(&mut self) {
        self.task_pool.push(tokio::spawn(async move {
            tokio::time::sleep(CONN_GRACE_PERIOD).await;
            AsyncTaskResult::Timeout
        }));
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

        // Schedule a timeout to drop connections in buffer.
        self.schedule_timeout();

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

    #[inline]
    pub fn cancel_dial(&mut self, dst: &NodeIndex) {
        if let Some(cancel) = self.pending_dial.remove(dst) {
            cancel.cancel();
        }
    }

    #[inline]
    fn spawn_connection_task(
        &mut self,
        connection: M::Connection,
        remote: NodeIndex,
    ) -> Sender<ServiceRequest> {
        let (request_tx, request_rx) = mpsc::channel(1024);
        let connection_id = connection.connection_id();
        let ctx = Context::new(connection, remote, request_rx, self.event_queue.clone());
        self.task_pool.push(tokio::spawn(async move {
            if let Err(e) = connection::connection_loop(ctx).await {
                tracing::info!("task for connection with {remote:?} exited with error: {e:?}");
            }
            AsyncTaskResult::ConnectionFinished {
                remote,
                connection_id,
            }
        }));

        request_tx
    }

    fn handle_new_connection(&mut self, connection: M::Connection) {
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
            let conn_request_sender = self.spawn_connection_task(connection, peer_index);

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

                    let request_tx_clone = conn_request_sender.clone();
                    if request_tx_clone.try_send(req).is_err() {
                        tracing::error!("failed to send pending request to connection task");
                    }
                }
            }

            // Save a handle to the connection task to send requests.
            let handle = OngoingConnectionHandle {
                service_request_tx: conn_request_sender,
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

    fn handle_stats_request(&mut self, _: oneshot::Sender<Stats>) -> anyhow::Result<()> {
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

    fn handle_finished_async_task(&mut self, task_result: AsyncTaskResult<M::Connection>) {
        match task_result {
            AsyncTaskResult::ConnectionSuccess { conn, .. } => {
                self.handle_new_connection(conn);
            },
            AsyncTaskResult::ConnectionFailed { remote, error } => {
                if remote.is_none() {
                    tracing::warn!("failed to connect to peer: {error:?}");
                } else {
                    let peer = remote.unwrap();
                    tracing::warn!("failed to dial peer {:?}: {error:?}", peer);
                    self.remove_pending_dial(&peer);
                }
            },
            AsyncTaskResult::ConnectionFinished {
                remote,
                connection_id,
            } => {
                tracing::trace!("task for connection={connection_id:?} with node={remote:?} ended");
                self.garbage_collect_closed_connections(remote, connection_id);
            },
            AsyncTaskResult::Timeout => {
                // Drop connections from last epoch.
                self.connection_buffer.clear();
            },
        }
    }

    /// Shutdowns workers and clears state.
    pub async fn shutdown(&mut self) {
        self.pool.clear();
        self.redundant_pool.clear();
        self.pending_task.clear();

        // All connection tasks should finished since we dropped all
        // Senders above.
        while self.task_pool.next().await.is_some() {}

        // We drop the muxer to unbind the address.
        self.muxer
            .take()
            .expect("start method to have been called")
            .close()
            .await;
    }

    async fn poll(&mut self, muxer: &mut M) {
        tokio::select! {
            next = muxer.accept() => {
                if let Some(connection) = next {
                    match connection {
                        Ok(connection) => {
                            let _ = self.handle_new_connection(connection);
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
            next = self.task_pool.next() => {
                if let Some(handle_result) = next {
                    match handle_result {
                        Ok(task_result) => {
                           let _ = self.handle_finished_async_task(task_result);
                        }
                        Err(e) => {
                            if e.is_panic() {
                                tracing::warn!("task panicked: {e:?}")
                            } else {
                                // Todo: when task IDs are stable in Tokio
                                // we could use them to track info
                                // about the task that failed
                                // for retries, clean-ups, etc.
                                tracing::warn!("task exited unexpectedly: {e:?}")
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn spawn(mut self) -> JoinHandle<Self> {
        tokio::spawn(async move {
            let _ = self.run().await;
            self
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut muxer = M::init(self.config.clone())?;
        self.muxer = Some(muxer.clone());
        let shutdown = self.shutdown.clone();

        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => {
                    break;
                }
                // Poll() is used so that branches can be polled randomly
                // but shutdown can still be polled first.
                _ = self.poll(&mut muxer) => {}
            }
        }

        Ok(())
    }
}

pub enum AsyncTaskResult<C: ConnectionInterface> {
    /// Connection attempt succeeded.
    ConnectionSuccess {
        incoming: bool,
        conn: C,
    },
    /// Connection attempt failed.
    ConnectionFailed {
        // Always Some when connection attempt
        // was outgoing and None otherwise.
        remote: Option<NodeIndex>,
        error: anyhow::Error,
    },
    /// Ongoing connection finished.
    ConnectionFinished {
        remote: NodeIndex,
        connection_id: ConnectionId,
    },
    Timeout,
}

pub struct OngoingConnectionHandle {
    pub(crate) service_request_tx: Sender<ServiceRequest>,
    pub(crate) connection_id: usize,
}
