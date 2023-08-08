//! In-memory mock global transport, backed by [`tokio::mspc::Sender`]s for incoming connections

use std::sync::Arc;

use anyhow::{anyhow, Result};
use dashmap::{mapref::entry::Entry, DashMap};
use fleek_crypto::NodePublicKey;
use lightning_schema::LightningMessage;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// Shared Memory Transport, should be cloned and passed to all ConnectionPool instances.
#[derive(Clone, Default)]
pub struct GlobalMemoryTransport<T> {
    /// Map of senders for connection pools to become aware of incoming connections
    nodes: Arc<DashMap<NodePublicKey, Sender<MemoryConnection<T>>>>,
}

impl<T: LightningMessage + 'static> GlobalMemoryTransport<T> {
    /// Connect to a node in the global transport. Returns None if the node is not found
    pub async fn connect(
        &self,
        src: NodePublicKey,
        dest: NodePublicKey,
    ) -> Option<MemoryConnection<T>> {
        let sender = self.nodes.get_mut(&dest)?;
        let (left, right) = MemoryConnection::pair(src, dest);
        sender
            .send(right)
            .await
            .expect("failed to send new connection to node pool");
        Some(left)
    }

    /// Register a new destination on the global transport
    pub async fn bind(
        &self,
        node: NodePublicKey,
        sender: Sender<MemoryConnection<T>>,
    ) -> Result<()> {
        let Entry::Vacant(entry) = self.nodes.entry(node) else {
            return Err(anyhow!("node already registered in the global transport"))
        };
        entry.insert(sender);
        Ok(())
    }
}

/// One end of a connection
pub struct MemoryConnection<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
    pub node: NodePublicKey,
}

impl<T: LightningMessage + 'static> MemoryConnection<T> {
    /// Create a new connection pair
    pub fn pair(left: NodePublicKey, right: NodePublicKey) -> (Self, Self) {
        let (tx0, rx0) = channel(32);
        let (tx1, rx1) = channel(32);

        (
            Self {
                sender: tx0,
                receiver: rx1,
                node: right,
            },
            Self {
                sender: tx1,
                receiver: rx0,
                node: left,
            },
        )
    }

    pub async fn send(&mut self, msg: T) -> Result<()> {
        self.sender.send(msg).await.map_err(|e| e.into())
    }

    pub async fn recv(&mut self) -> Option<T> {
        self.receiver.recv().await
    }
}
