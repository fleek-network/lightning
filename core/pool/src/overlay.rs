use std::cell::OnceCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::SocketAddr;

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, ServiceScope, SyncQueryRunnerInterface};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use x509_parser::nom::AsBytes;

use crate::endpoint::NodeAddress;
use crate::muxer::{Channel, ConnectionInterface, MuxerInterface};

pub type BoxedFilterCallback = Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>;

pub struct NetworkOverlay<C, F = BoxedFilterCallback>
where
    C: Collection,
    F: Fn(NodeIndex) -> bool,
{
    /// Peers that we are currently connected to.
    peers: HashMap<NodeIndex, ConnectionInfo>,
    /// Service handles.
    broadcast_handles: HashMap<ServiceScope, Sender<(NodeIndex, Bytes)>>,
    /// Receive requests for broadcast service.
    broadcast_request_rx: Receiver<BroadcastRequest<F>>,
    /// Sender to return to users as they register with the broadcast service.
    broadcast_request_tx: Sender<BroadcastRequest<F>>,
    /// Service handles.
    stream_handles: HashMap<ServiceScope, Sender<(NodeIndex, Channel)>>,
    /// Receive requests for a multiplexed stream.
    stream_request_rx: Receiver<ChannelRequest>,
    /// Send handle to return to users as they register with the stream service.
    stream_request_tx: Sender<ChannelRequest>,
    /// Sync query runner.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Local node index.
    index: OnceCell<NodeIndex>,
    /// Local node public key.
    pk: NodePublicKey,
}

impl<C> NetworkOverlay<C>
where
    C: Collection,
{
    pub fn new(
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        pk: NodePublicKey,
        index: OnceCell<NodeIndex>,
    ) -> Self {
        let (broadcast_request_tx, broadcast_request_rx) = mpsc::channel(1024);
        let (stream_request_tx, stream_request_rx) = mpsc::channel(1024);
        Self {
            broadcast_handles: HashMap::new(),
            stream_handles: HashMap::new(),
            peers: HashMap::new(),
            broadcast_request_tx,
            broadcast_request_rx,
            stream_request_tx,
            stream_request_rx,
            sync_query,
            index,
            pk,
        }
    }

    pub fn register_broadcast_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<BroadcastRequest>, Receiver<(NodeIndex, Bytes)>) {
        let (tx, rx) = mpsc::channel(1024);
        self.broadcast_handles.insert(service_scope, tx);
        (self.broadcast_request_tx.clone(), rx)
    }

    pub fn register_stream_service(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<ChannelRequest>, Receiver<(NodeIndex, Channel)>) {
        let (tx, rx) = mpsc::channel(1024);
        self.stream_handles.insert(service_scope, tx);
        (self.stream_request_tx.clone(), rx)
    }

    pub fn handle_broadcast_message(&mut self, peer: NodeIndex, event: Message) {
        if !self.peers.contains_key(&peer) {
            return;
        }

        let Message {
            service: service_scope,
            payload: message,
        } = event;

        if let Some(tx) = self.broadcast_handles.get(&service_scope).cloned() {
            tokio::spawn(async move {
                if tx.send((peer, Bytes::from(message))).await.is_err() {
                    tracing::error!("failed to send message to user");
                }
            });
        }
    }

    pub fn handle_incoming_stream(
        &mut self,
        peer: NodeIndex,
        service_scope: ServiceScope,
        stream: Channel,
    ) {
        match self.stream_handles.get(&service_scope).cloned() {
            None => {
                tracing::warn!("received unknown service scope: {service_scope:?}");
            },
            Some(tx) => {
                if let Some(address) = self.node_address_from_state(&peer) {
                    self.pin_connection(peer, address);
                    tokio::spawn(async move {
                        if tx.send((peer, stream)).await.is_err() {
                            tracing::error!("failed to send incoming stream to user");
                        }
                    });
                }
            },
        }
    }

    #[inline]
    pub fn contains(&self, peer: &NodeIndex) -> bool {
        self.peers.contains_key(peer)
    }

    #[inline]
    pub fn update_connections(&mut self, peers: HashSet<NodeIndex>) -> BroadcastTask {
        // We keep pinned connections.
        self.peers.retain(|index, info| {
            let is_in_overlay = peers.contains(index);
            if is_in_overlay {
                // We want to update the flag in case it wasn't initially created by a
                // topology update.
                info.from_topology = true
            }
            is_in_overlay || info.pinned
        });

        // We get information about the peers.
        let peers = peers
            .into_iter()
            // We ignore connections for which we don't have information on state.
            .filter_map(|index| {
                let address = self.node_address_from_state(&index)?;
                let should_connect = address.index < self.get_index();
                Some(ConnectionInfo {
                    from_topology: true,
                    pinned: false,
                    address,
                    connect: should_connect,
                })
            })
            .collect::<Vec<_>>();

        // We perform a union.
        for info in peers.iter() {
            self.peers.entry(info.address.index).or_insert(info.clone());
        }

        // We tell the pool who to connect to.
        BroadcastTask::Update {
            peers: self.peers.clone(),
        }
    }

    pub fn get_index(&self) -> NodeIndex {
        if let Some(index) = self.index.get() {
            *index
        } else if let Some(index) = self.sync_query.pubkey_to_index(self.pk) {
            self.index.set(index).expect("Failed to set index");
            index
        } else {
            u32::MAX
        }
    }

    pub fn node_address_from_state(&self, index: &NodeIndex) -> Option<NodeAddress> {
        let pk = self.sync_query.index_to_pubkey(*index)?;

        let info = self.sync_query.get_node_info(&pk)?;

        Some(NodeAddress {
            index: *index,
            pk,
            socket_address: SocketAddr::from((info.domain, info.ports.pool)),
        })
    }

    pub fn pin_connection(&mut self, peer: NodeIndex, address: NodeAddress) {
        let our_index = self.get_index();
        self.peers
            .entry(peer)
            .and_modify(|info| info.pinned = true)
            .or_insert({
                let should_connect = peer < our_index;
                ConnectionInfo {
                    pinned: true,
                    from_topology: false,
                    address,
                    connect: should_connect,
                }
            });
    }

    /// Cleans up a pinned connection.
    ///
    /// This method assumes that there are no active transport connections
    /// before calling it because we could potentially erroneously unpin a connection.
    pub fn clean(&mut self, peer: NodeIndex) {
        if let Entry::Occupied(mut entry) = self.peers.entry(peer) {
            let info = entry.get_mut();
            if !info.from_topology && info.pinned {
                entry.remove();
            } else if info.pinned {
                info.pinned = false;
            }
        }
    }

    /// Returns true if the peer has staked the required amount
    /// to be a valid node in the network, and false otherwise.
    #[inline]
    pub fn validate_stake(&self, peer: NodePublicKey) -> bool {
        HpUfixed::from(self.sync_query.get_staking_amount()) <= self.sync_query.get_staked(&peer)
    }

    pub fn _index_from_connection<M: MuxerInterface>(
        &self,
        connection: &M::Connection,
    ) -> Option<NodeIndex> {
        let pk = connection.peer_identity()?;
        self.sync_query.pubkey_to_index(pk)
    }

    pub fn pubkey_to_index(&self, key: NodePublicKey) -> Option<NodeIndex> {
        self.sync_query.pubkey_to_index(key)
    }

    pub fn _index_to_pubkey(&self, index: NodeIndex) -> Option<NodePublicKey> {
        self.sync_query.index_to_pubkey(index)
    }

    pub async fn next(&mut self) -> Option<PoolTask> {
        loop {
            tokio::select! {
                stream_request = self.stream_request_rx.recv() => {
                    let request = stream_request?;
                    match self.node_address_from_state(&request.peer) {
                        Some(address) => {
                            self.pin_connection(request.peer, address.clone());
                            return Some(PoolTask::Channel(ChannelTask {
                                    peer: address,
                                    service_scope: request.service_scope,
                                    respond: request.respond,
                                }));
                        }
                        None => {
                            if request.respond.send(
                                Err(io::ErrorKind::AddrNotAvailable.into())
                            ).is_err() {
                                tracing::error!("respond channel was closed")
                            }
                        }
                    }
                }
                broadcast_request = self.broadcast_request_rx.recv() => {
                    let request = broadcast_request?;
                    let peers = match request.param {
                        Param::Filter(filter) => {
                            Some(self
                                .peers
                                .iter()
                                .filter(|(index, _)| filter(**index))
                                .map(|(_, info)| info.clone())
                                .collect::<Vec<_>>())
                        }
                        Param::Index(index) => {
                            self.peers.get(&index).map(|info| vec![info.clone()])
                        },
                    };

                    if let Some(peers) = peers {
                        let peers = if peers.is_empty() { None } else { Some(peers) };

                        return Some(PoolTask::Broadcast(BroadcastTask::Send {
                            service_scope: request.service_scope,
                            message: request.message,
                            peers,
                        }));
                    }
                }
            }
            tokio::task::yield_now().await;
        }
    }
}

pub enum PoolTask {
    Broadcast(BroadcastTask),
    Channel(ChannelTask),
}

pub struct BroadcastRequest<F = BoxedFilterCallback>
where
    F: Fn(NodeIndex) -> bool,
{
    pub service_scope: ServiceScope,
    pub message: Bytes,
    pub param: Param<F>,
}

pub enum Param<F>
where
    F: Fn(NodeIndex) -> bool,
{
    Filter(F),
    Index(NodeIndex),
}

pub enum BroadcastTask {
    Send {
        service_scope: ServiceScope,
        message: Bytes,
        peers: Option<Vec<ConnectionInfo>>,
    },
    Update {
        // Nodes that are in our overlay.
        peers: HashMap<NodeIndex, ConnectionInfo>,
    },
}

pub struct ChannelTask {
    pub peer: NodeAddress,
    pub service_scope: ServiceScope,
    pub respond: oneshot::Sender<io::Result<Channel>>,
}

pub struct ChannelRequest {
    pub service_scope: ServiceScope,
    pub peer: NodeIndex,
    pub respond: oneshot::Sender<io::Result<Channel>>,
}

#[derive(Clone, Debug)]
pub struct ConnectionInfo {
    /// Pinned connections should not be dropped
    /// on topology changes.
    pub pinned: bool,
    /// This connection was initiated on a topology event.
    pub from_topology: bool,
    /// The address of the peer.
    pub address: NodeAddress,
    /// This field is used during topology updates.
    /// It tells the pool whether it should connect
    /// to the peer or wait for the peer to connect.
    pub connect: bool,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub service: ServiceScope,
    pub payload: Vec<u8>,
}

impl From<Message> for Bytes {
    fn from(value: Message) -> Self {
        let mut buf = BytesMut::with_capacity(value.payload.len() + 1);
        buf.put_u8(value.service as u8);
        buf.put_slice(&value.payload);
        buf.into()
    }
}

impl TryFrom<BytesMut> for Message {
    type Error = anyhow::Error;

    fn try_from(value: BytesMut) -> anyhow::Result<Self> {
        let bytes = value.as_bytes();
        if bytes.is_empty() {
            return Err(anyhow::anyhow!("Cannot convert empty bytes into a message"));
        }
        let service = ServiceScope::try_from(bytes[0])?;
        let payload = bytes[1..bytes.len()].to_vec();
        Ok(Self { service, payload })
    }
}
