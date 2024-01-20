//
// pub struct NetworkOverlay<C, F = BoxedFilterCallback>
// where
//     C: Collection,
//     F: Fn(NodeIndex) -> bool,
// {
//     /// Peers that we are currently connected to.
//     pub(crate) peers: HashMap<NodeIndex, ConnectionInfo>,
//     /// Service handles.
//     broadcast_service_handles: HashMap<ServiceScope, Sender<(NodeIndex, Bytes)>>,
//     /// Receive requests for broadcast service.
//     broadcast_request_rx: Receiver<BroadcastRequest<F>>,
//     /// Sender to return to users as they register with the broadcast service.
//     broadcast_request_tx: Sender<BroadcastRequest<F>>,
//     /// Service handles.
//     send_request_service_handles: HashMap<ServiceScope, Sender<(RequestHeader, Request)>>,
//     /// Receive requests for a multiplexed stream.
//     send_request_rx: Receiver<SendRequest>,
//     /// Send handle to return to users as they register with the send-request service.
//     send_request_tx: Sender<SendRequest>,
//     /// Sync query runner.
//     sync_query: c![C::ApplicationInterface::SyncExecutor],
//     /// Local node index.
//     index: OnceCell<NodeIndex>,
//     /// Stats.
//     stats: Stats,
//     /// Local node public key.
//     pk: NodePublicKey,
// }
//
// impl<C> NetworkOverlay<C>
// where
//     C: Collection,
// {
//     pub fn new(
//         sync_query: c!(C::ApplicationInterface::SyncExecutor),
//         pk: NodePublicKey,
//         index: OnceCell<NodeIndex>,
//     ) -> Self {
//         let (broadcast_request_tx, broadcast_request_rx) = mpsc::channel(1024);
//         let (stream_request_tx, stream_request_rx) = mpsc::channel(1024);
//         Self {
//             broadcast_service_handles: HashMap::new(),
//             send_request_service_handles: HashMap::new(),
//             peers: HashMap::new(),
//             broadcast_request_tx,
//             broadcast_request_rx,
//             send_request_tx: stream_request_tx,
//             send_request_rx: stream_request_rx,
//             sync_query,
//             index,
//             stats: Stats::default(),
//             pk,
//         }
//     }
//
//     pub fn register_broadcast_service(
//         &mut self,
//         service_scope: ServiceScope,
//     ) -> (Sender<BroadcastRequest>, Receiver<(NodeIndex, Bytes)>) {
//         let (tx, rx) = mpsc::channel(1024);
//         self.broadcast_service_handles.insert(service_scope, tx);
//         (self.broadcast_request_tx.clone(), rx)
//     }
//
//     pub fn register_requester_service(
//         &mut self,
//         service_scope: ServiceScope,
//     ) -> (Sender<SendRequest>, Receiver<(RequestHeader, Request)>) {
//         let (tx, rx) = mpsc::channel(1024);
//         self.send_request_service_handles.insert(service_scope, tx);
//         (self.send_request_tx.clone(), rx)
//     }
//
//     pub fn handle_broadcast_message(&mut self, peer: NodeIndex, event: Message) {
//         if !self.peers.contains_key(&peer) {
//             return;
//         }
//
//         let Message {
//             service: service_scope,
//             payload: message,
//         } = event;
//
//         if let Some(tx) = self.broadcast_service_handles.get(&service_scope).cloned() {
//             tokio::spawn(async move {
//                 if tx.send((peer, Bytes::from(message))).await.is_err() {
//                     tracing::error!("failed to send message to user");
//                 }
//             });
//         }
//     }
//
//     pub fn handle_incoming_request(
//         &mut self,
//         peer: NodeIndex,
//         service_scope: ServiceScope,
//         request: (RequestHeader, Request),
//     ) {
//         match self
//             .send_request_service_handles
//             .get(&service_scope)
//             .cloned()
//         {
//             None => {
//                 tracing::warn!("received unknown service scope: {service_scope:?}");
//             },
//             Some(tx) => {
//                 if let Some(info) = self.node_info_from_state(&peer) {
//                     self.pin_connection(peer, info);
//                     tokio::spawn(async move {
//                         if tx.send(request).await.is_err() {
//                             tracing::error!("failed to send incoming request to user");
//                         }
//                     });
//                 }
//             },
//         }
//     }
//
//     #[inline]
//     pub fn contains(&self, peer: &NodeIndex) -> bool {
//         self.peers.contains_key(peer)
//     }
//
//     #[inline]
//     pub fn update_connections(&mut self, peers: HashSet<NodeIndex>) -> BroadcastTask {
//         // We keep pinned connections.
//         let mut peers_to_drop = Vec::new();
//         self.peers.retain(|index, info| {
//             let is_in_overlay = peers.contains(index);
//             if is_in_overlay {
//                 // We want to update the flag in case it wasn't initially created by a
//                 // topology update.
//                 info.from_topology = true
//             }
//
//             if is_in_overlay || info.pinned {
//                 true
//             } else {
//                 peers_to_drop.push(*index);
//                 false
//             }
//         });
//
//         // We get information about the peers.
//         let peers = peers
//             .into_iter()
//             // We ignore connections for which we don't have information on state.
//             .filter_map(|index| {
//                 let info = self.node_info_from_state(&index)?;
//                 let should_connect = info.index < self.get_index();
//                 Some(ConnectionInfo {
//                     from_topology: true,
//                     pinned: false,
//                     node_info: info,
//                     connect: should_connect,
//                 })
//             })
//             .collect::<Vec<_>>();
//
//         // We perform a union.
//         for info in peers.iter() {
//             self.peers
//                 .entry(info.node_info.index)
//                 .or_insert(info.clone());
//         }
//
//         // Report stats.
//         histogram!(
//             "pool_neighborhood_size",
//             Some("Size of peers (not pinned and from topology) in our neighborhood."),
//             peers.len() as f64
//         );
//
//         // We would like to measure how many times we are
//         // using request-response service with peers in our cluster.
//         histogram!(
//             "pool_total_req_res_requests",
//             Some("Total number of req-res requests"),
//             self.stats.total_req_res_requests as f64
//         );
//         // Reset.
//         self.stats.total_req_res_requests = 0;
//
//         histogram!(
//             "pool_cluster_hit",
//             Some(
//                 "Number of times that we are using request-response \
//                 service with peers in our cluster"
//             ),
//             self.stats.cluster_hit_count as f64
//         );
//         // Reset.
//         self.stats.cluster_hit_count = 0;
//
//         // We tell the pool who to connect to.
//         BroadcastTask::Update {
//             keep: self.peers.clone(),
//             drop: peers_to_drop,
//         }
//     }
//
//     pub fn get_index(&self) -> NodeIndex {
//         if let Some(index) = self.index.get() {
//             *index
//         } else if let Some(index) = self.sync_query.pubkey_to_index(&self.pk) {
//             self.index.set(index).expect("Failed to set index");
//             index
//         } else {
//             u32::MAX
//         }
//     }
//
//     pub fn node_info_from_state(&self, peer: &NodeIndex) -> Option<NodeInfo> {
//         let info = self
//             .sync_query
//             .get_node_info::<lightning_interfaces::types::NodeInfo>(peer, |n| n)?;
//
//         Some(NodeInfo {
//             index: *peer,
//             pk: info.public_key,
//             socket_address: SocketAddr::from((info.domain, info.ports.pool)),
//         })
//     }
//
//     pub fn pin_connection(&mut self, peer: NodeIndex, info: NodeInfo) {
//         let our_index = self.get_index();
//         self.peers
//             .entry(peer)
//             .and_modify(|info| info.pinned = true)
//             .or_insert({
//                 let should_connect = peer < our_index;
//                 ConnectionInfo {
//                     pinned: true,
//                     from_topology: false,
//                     node_info: info,
//                     connect: should_connect,
//                 }
//             });
//     }
//
//     /// Cleans up a pinned connection.
//     ///
//     /// This method assumes that there are no active transport connections
//     /// before calling it because we could potentially erroneously unpin a connection.
//     pub fn clean(&mut self, peer: NodeIndex) {
//         if let Entry::Occupied(mut entry) = self.peers.entry(peer) {
//             let info = entry.get_mut();
//             if !info.from_topology && info.pinned {
//                 entry.remove();
//             } else if info.pinned {
//                 info.pinned = false;
//             }
//         }
//     }
//
//     /// Returns true if the peer has staked the required amount
//     /// to be a valid node in the network, and false otherwise.
//     #[inline]
//     pub fn validate_stake(&self, peer: NodePublicKey) -> bool {
//         match self.sync_query.pubkey_to_index(&peer) {
//             None => false,
//             Some(ref node_idx) => {
//                 HpUfixed::from(self.sync_query.get_staking_amount())
//                     <= self
//                         .sync_query
//                         .get_node_info::<HpUfixed<18>>(node_idx, |n| n.stake.staked)
//                         .unwrap_or(HpUfixed::<18>::zero())
//             },
//         }
//     }
//
//     pub fn _index_from_connection<M: MuxerInterface>(
//         &self,
//         connection: &M::Connection,
//     ) -> Option<NodeIndex> {
//         let pk = connection.peer_identity()?;
//         self.sync_query.pubkey_to_index(&pk)
//     }
//
//     pub fn pubkey_to_index(&self, peer: NodePublicKey) -> Option<NodeIndex> {
//         self.sync_query.pubkey_to_index(&peer)
//     }
//
//     pub fn _index_to_pubkey(&self, peer: NodeIndex) -> Option<NodePublicKey> {
//         self.sync_query.index_to_pubkey(&peer)
//     }
//
//     #[inline]
//     pub fn record_req_res_request(&mut self, peer: &NodeIndex) {
//         self.stats.total_req_res_requests += 1;
//
//         if let Some(info) = self.peers.get(peer) {
//             // If it's pinned, it means it's already been accounted for.
//             if info.from_topology && !info.pinned {
//                 self.stats.cluster_hit_count += 1;
//             }
//         }
//     }
//
//     #[inline]
//     pub fn connections(&self) -> HashMap<NodeIndex, ConnectionInfo> {
//         self.peers.clone()
//     }
//
//     #[inline]
//     pub fn get_connection_info(&self, peer: &NodeIndex) -> Option<&ConnectionInfo> {
//         self.peers.get(peer)
//     }
//
//     #[inline]
//     pub fn broadcast_queue_cap(&self) -> (usize, usize) {
//         (
//             self.broadcast_request_tx.capacity(),
//             self.broadcast_request_tx.max_capacity(),
//         )
//     }
//
//     #[inline]
//     pub fn send_req_queue_cap(&self) -> (usize, usize) {
//         (
//             self.send_request_tx.capacity(),
//             self.send_request_tx.max_capacity(),
//         )
//     }
//
//     pub async fn next(&mut self) -> Option<PoolTask> {
//         loop {
//             tokio::select! {
//                 send_request = self.send_request_rx.recv() => {
//                     let send_request = send_request?;
//                     match self.node_info_from_state(&send_request.peer) {
//                         Some(info) => {
//                             self.record_req_res_request(&send_request.peer);
//                             self.pin_connection(send_request.peer, info.clone());
//                             return Some(PoolTask::SendRequest(SendRequestTask {
//                                     peer: info,
//                                     service_scope: send_request.service_scope,
//                                     request: send_request.request,
//                                     respond: send_request.respond,
//                                 }));
//                         }
//                         None => {
//                             if send_request.respond.send(
//                                 Err(io::ErrorKind::AddrNotAvailable.into())
//                             ).is_err() {
//                                 tracing::error!("requester dropped the channel")
//                             }
//                         }
//                     }
//                 }
//                 broadcast_request = self.broadcast_request_rx.recv() => {
//                     let request = broadcast_request?;
//                     let peers: Vec<ConnectionInfo> = match request.param {
//                         Param::Filter(filter) => {
//                             self
//                                 .peers
//                                 .iter()
//                                 .filter(|(index, info)| info.from_topology && filter(**index))
//                                 .map(|(_, info)| info.clone())
//                                 .collect()
//                         }
//                         Param::Index(index) => {
//                             self
//                                 .peers
//                                 .get(&index)
//                                 .filter(|info| info.from_topology)
//                                 .map(|info| vec![info.clone()])
//                                 .unwrap_or_default()
//                         },
//                     };
//
//                     if !peers.is_empty()  {
//                         return Some(PoolTask::Broadcast(BroadcastTask::Send {
//                             service_scope: request.service_scope,
//                             message: request.message,
//                             peers,
//                         }));
//                     } else {
//                         tracing::warn!("no peers to send the message to: no peers in state or
// filter was too restrictive");                     }
//                 }
//             }
//             tokio::task::yield_now().await;
//         }
//     }
// }
