use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::net::UdpSocket;

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
pub struct NetSocket {
    inner: Arc<UdpSocket>,
}

impl NetSocket {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        Self { inner: socket }
    }

    pub async fn recv_from(&self) -> std::io::Result<(Bytes, SocketAddr)> {
        // Todo: Let's make sure that our messages can fit in one datagram.
        let mut buf = vec![0u8; 64 * 1024];
        let (size, address) = self.inner.recv_from(&mut buf).await?;
        buf.truncate(size);
        Ok((buf.into(), address))
    }

    pub async fn send_to(&self, buf: &[u8], peer: SocketAddr) -> std::io::Result<()> {
        self.inner.send_to(buf, peer).await?;
        Ok(())
    }
}
