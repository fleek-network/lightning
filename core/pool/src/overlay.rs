use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::SocketAddr;

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodePublicKey;
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
    stream_handles: HashMap<ServiceScope, Sender<Channel>>,
    /// Receive requests for a multiplexed stream.
    stream_request_rx: Receiver<StreamRequest>,
    /// Send handle to return to users as they register with the stream service.
    stream_request_tx: Sender<StreamRequest>,
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
    ) -> (Sender<StreamRequest>, Receiver<Channel>) {
        let (tx, rx) = mpsc::channel(1024);
        self.stream_handles.insert(service_scope, tx);
        (self.stream_request_tx.clone(), rx)
    }

    pub fn handle_broadcast_message(&mut self, peer: NodeIndex, event: Message) {
        // Todo: Validate if peer is in our overlay
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

    // Todo: Pass the public key so we can validate the peer.
    pub fn handle_incoming_stream(&self, service_scope: ServiceScope, stream: Channel) -> bool {
        match self.stream_handles.get(&service_scope).cloned() {
            None => {
                tracing::warn!("received unknown service scope: {service_scope:?}");
                false
            },
            Some(tx) => {
                tokio::spawn(async move {
                    if tx.send(stream).await.is_err() {
                        tracing::error!("failed to send incoming stream to user");
                    }
                });
                true
            },
        }
    }

    #[inline]
    pub fn is_redundant(&self, peer: &NodeIndex) -> bool {
        self.peers.contains_key(peer) && peer > &self.get_index()
    }

    #[inline]
    pub fn contains(&self, peer: &NodeIndex) -> bool {
        self.peers.contains_key(peer)
    }

    #[inline]
    pub fn update_connections(&mut self, peers: HashSet<NodeIndex>) -> BroadcastTask {
        // We keep pinned connections as well.
        self.peers
            .retain(|index, info| peers.contains(index) || info.pinned);

        let peers = peers
            .into_iter()
            .filter_map(|index| {
                let address = self.node_address_from_state(&index)?;
                Some(ConnectionInfo {
                    pinned: false,
                    address,
                })
            })
            .collect::<Vec<_>>();

        for info in peers.iter() {
            self.peers.entry(info.address.index).or_insert(info.clone());
        }

        let updated = self
            .peers
            .values()
            .map(|info| {
                let should_connect = info.address.index < self.get_index();
                (
                    info.address.index,
                    ConnectionUpdate {
                        address: info.address.clone(),
                        connect: should_connect,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        BroadcastTask::Update { peers: updated }
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

    fn node_address_from_state(&self, index: &NodeIndex) -> Option<NodeAddress> {
        let pk = self.sync_query.index_to_pubkey(*index)?;

        let info = self.sync_query.get_node_info(&pk)?;

        Some(NodeAddress {
            index: *index,
            pk,
            socket_address: SocketAddr::from((info.domain, info.ports.pool)),
        })
    }

    pub fn pin_connection(&mut self, peer: NodeIndex, address: NodeAddress) {
        self.peers
            .entry(peer)
            .and_modify(|info| info.pinned = true)
            .or_insert(ConnectionInfo {
                pinned: true,
                address,
            });
    }

    pub fn index_from_connection<M: MuxerInterface>(
        &self,
        connection: &M::Connection,
    ) -> Option<NodeIndex> {
        let pk = connection.peer_identity()?;
        self.sync_query.pubkey_to_index(pk)
    }

    pub fn pubkey_to_index(&self, key: NodePublicKey) -> Option<NodeIndex> {
        self.sync_query.pubkey_to_index(key)
    }

    pub async fn next(&mut self) -> Option<PoolTask> {
        loop {
            tokio::select! {
                stream_request = self.stream_request_rx.recv() => {
                    let request = stream_request?;
                    match self.node_address_from_state(&request.peer) {
                        Some(address) => {
                            self.pin_connection(request.peer, address.clone());
                            return Some(PoolTask::Stream(StreamTask {
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
                        Param::Filter(filter) => self
                            .peers
                            .iter()
                            .filter(|(index, _)| filter(**index))
                            .map(|(_, info)| info.clone())
                            .collect::<Vec<_>>(),
                        Param::Index(index) => {
                            let Some(address) = self.node_address_from_state(&index) else {
                                tracing::error!("failed to find address for peer {index:?}");
                                continue;
                            };
                            vec![ConnectionInfo { pinned: false, address }]
                        },
                    };

                    let peers = if peers.is_empty() { None } else { Some(peers) };

                    return Some(PoolTask::Broadcast(BroadcastTask::Send {
                        service_scope: request.service_scope,
                        message: request.message,
                        peers,
                    }));
                }
            }
            tokio::task::yield_now().await;
        }
    }
}

pub enum PoolTask {
    Broadcast(BroadcastTask),
    Stream(StreamTask),
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
        peers: HashMap<NodeIndex, ConnectionUpdate>,
    },
}

pub struct StreamTask {
    pub peer: NodeAddress,
    pub service_scope: ServiceScope,
    pub respond: oneshot::Sender<io::Result<Channel>>,
}

pub struct StreamRequest {
    pub service_scope: ServiceScope,
    pub peer: NodeIndex,
    pub respond: oneshot::Sender<io::Result<Channel>>,
}

pub struct ConnectionUpdate {
    pub address: NodeAddress,
    pub connect: bool,
}

#[derive(Clone)]
pub struct ConnectionInfo {
    pub pinned: bool,
    pub address: NodeAddress,
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
