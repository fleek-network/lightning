use std::io;
use std::net::SocketAddrV4;

use bytes::Bytes;
use tokio::net::UnixStream;

use crate::frame::{IpcServiceFrame, Pf, PACKET_FILTER_SERVICE};
use crate::utils::write;

/// Client to update dynamic list of packet filter rules.
#[derive(Debug)]
pub struct PacketFilter {
    inner: UnixStream,
}

impl PacketFilter {
    pub fn new(&mut self, stream: UnixStream) -> Self {
        Self { inner: stream }
    }

    pub async fn connect(&self) -> io::Result<()> {
        let service = PACKET_FILTER_SERVICE.to_le_bytes();
        write(&self.inner, Bytes::from(service.to_vec())).await
    }

    pub async fn add(&self, addr: SocketAddrV4) -> io::Result<()> {
        let frame = IpcServiceFrame::Pf(Pf { op: Pf::ADD, addr });
        write(&self.inner, frame.serialize_len_delimit()).await
    }

    pub async fn remove(&self, addr: SocketAddrV4) -> io::Result<()> {
        let frame = IpcServiceFrame::Pf(Pf {
            op: Pf::REMOVE,
            addr,
        });
        write(&self.inner, frame.serialize_len_delimit()).await
    }
}
