use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::SyncQueryRunnerInterface;
use tokio::net::UdpSocket;

use crate::network::message::Message;
use crate::network::transport::UnreliableTransport;

#[derive(Clone)]
pub struct UdpTransport<S> {
    socket: Arc<UdpSocket>,
    sync_query: S,
}

impl<S: SyncQueryRunnerInterface> UdpTransport<S> {
    pub fn new(socket: Arc<UdpSocket>, sync_query: S) -> Self {
        Self { socket, sync_query }
    }

    pub async fn send(&self, buf: &[u8], peer: SocketAddr) -> std::io::Result<()> {
        self.socket.send_to(buf, peer).await?;
        Ok(())
    }

    pub async fn inner_recv(&self) -> std::io::Result<(Bytes, SocketAddr)> {
        let mut buf = vec![0u8; 64 * 1024];
        let (size, address) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(size);
        Ok((buf.into(), address))
    }
}

#[async_trait]
impl<S: SyncQueryRunnerInterface> UnreliableTransport for UdpTransport<S> {
    async fn send(&self, message: Message, dst: NodeIndex) -> std::io::Result<()> {
        // Todo: Let's make sure that our messages can fit in one datagram.
        let bytes = message.encode();
        let info = self
            .sync_query
            .get_node_info_with_index(&dst)
            .ok_or(io::Error::from(io::ErrorKind::AddrNotAvailable))?;
        let address = SocketAddr::new(info.domain, info.ports.dht);
        self.send(bytes.as_ref(), address).await
    }

    // This is cancel-safe.
    async fn recv(&self) -> std::io::Result<(NodeIndex, Message)> {
        let (bytes, _) = self.inner_recv().await?;
        let message = Message::decode(bytes).map_err(|_| io::ErrorKind::InvalidData)?;
        // Todo: Find a trust-less way of deriving the node index.
        // It might be good to abstract it so we don't have to implement it for every transport.
        Ok((message.from, message))
    }
}
