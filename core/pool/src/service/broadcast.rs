use std::collections::{HashMap, HashSet};

use bytes::{BufMut, Bytes, BytesMut};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::ServiceScope;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use x509_parser::nom::AsBytes;

pub struct BroadcastService<F>
where
    F: Fn(NodeIndex) -> bool,
{
    /// Service handles.
    handles: HashMap<ServiceScope, Sender<(NodeIndex, Bytes)>>,
    /// Peers that we are currently connected to.
    peers: HashSet<NodeIndex>,
    /// Receive requests for broadcast service.
    request_rx: Receiver<BroadcastRequest<F>>,
    /// Sender to return to users as they register with the broadcast service.
    request_tx: Sender<BroadcastRequest<F>>,
}

impl<F> BroadcastService<F>
where
    F: Fn(NodeIndex) -> bool,
{
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel(1024);
        Self {
            handles: HashMap::new(),
            peers: HashSet::new(),
            request_tx,
            request_rx,
        }
    }

    pub fn register(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<BroadcastRequest<F>>, Receiver<(NodeIndex, Bytes)>) {
        let (tx, rx) = mpsc::channel(1024);
        self.handles.insert(service_scope, tx);
        (self.request_tx.clone(), rx)
    }

    pub fn handle_broadcast_message(&mut self, peer: NodeIndex, event: Message) {
        let Message {
            service: service_scope,
            payload: message,
        } = event;

        if let Some(tx) = self.handles.get(&service_scope).cloned() {
            tokio::spawn(async move {
                if tx.send((peer, Bytes::from(message))).await.is_err() {
                    tracing::error!("failed to send message to user");
                }
            });
        }
    }

    #[inline]
    pub fn contains(&self, peer: &NodeIndex) -> bool {
        self.peers.contains(peer)
    }

    #[inline]
    pub fn update_connections(&mut self, peers: HashSet<NodeIndex>) -> BroadcastTask {
        let (keep, disconnect) = match self.peers.is_empty() {
            true => (peers, HashSet::new()),
            false => self
                .peers
                .union(&peers)
                .partition(|index| peers.contains(index)),
        };
        self.peers = keep.clone();
        BroadcastTask::Update { keep, disconnect }
    }

    pub async fn next(&mut self) -> Option<BroadcastTask> {
        let request = self.request_rx.recv().await?;
        let peers = match request.param {
            Param::Filter(filter) => self
                .peers
                .iter()
                .copied()
                .filter(|index| filter(*index))
                .collect::<HashSet<_>>(),
            Param::Index(index) => {
                let mut set = HashSet::new();
                set.insert(index);
                set
            },
        };

        let peers = if peers.is_empty() { None } else { Some(peers) };

        Some(BroadcastTask::Send {
            service_scope: request.service_scope,
            message: request.message,
            peers,
        })
    }
}

pub enum Param<F>
where
    F: Fn(NodeIndex) -> bool,
{
    Filter(F),
    Index(NodeIndex),
}

pub type BoxedFilterCallback = Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>;

pub struct BroadcastRequest<F = BoxedFilterCallback>
where
    F: Fn(NodeIndex) -> bool,
{
    pub service_scope: ServiceScope,
    pub message: Bytes,
    pub param: Param<F>,
}

pub enum BroadcastTask {
    Send {
        service_scope: ServiceScope,
        message: Bytes,
        peers: Option<HashSet<NodeIndex>>,
    },
    Update {
        keep: HashSet<NodeIndex>,
        disconnect: HashSet<NodeIndex>,
    },
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
