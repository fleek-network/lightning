use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::net::UdpSocket;

use crate::network::message::Message;
use crate::network::transport::UnreliableTransport;

pub async fn recv_from(socket: &UdpSocket) -> std::io::Result<(Vec<u8>, SocketAddr)> {
    // Todo: Let's make sure that our messages can fit in one datagram.
    let mut buf = vec![0u8; 64 * 1024];
    let (size, address) = socket.recv_from(&mut buf).await?;
    buf.truncate(size);
    Ok((buf, address))
}

pub async fn send_to(socket: &UdpSocket, buf: &[u8], peer: SocketAddr) -> std::io::Result<()> {
    socket.send_to(buf, peer).await?;
    Ok(())
}

// Todo: Add chunking.
#[derive(Clone)]
pub struct UdpTransport {
    inner: Arc<UdpSocket>,
}

impl UdpTransport {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        Self { inner: socket }
    }

    pub async fn send(&self, buf: &[u8], peer: SocketAddr) -> std::io::Result<()> {
        self.inner.send_to(buf, peer).await?;
        Ok(())
    }

    pub async fn recv(&self) -> std::io::Result<(Bytes, SocketAddr)> {
        // Todo: Let's make sure that our messages can fit in one datagram.
        let mut buf = vec![0u8; 64 * 1024];
        let (size, address) = self.inner.recv_from(&mut buf).await?;
        buf.truncate(size);
        Ok((buf.into(), address))
    }
}

#[async_trait]
impl UnreliableTransport for UdpTransport {
    async fn send(&self, message: Message) -> std::io::Result<()> {
        todo!()
    }

    async fn recv(&self) -> std::io::Result<Message> {
        todo!()
    }
}
