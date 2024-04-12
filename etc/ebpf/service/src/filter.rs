use std::io;
use std::net::SocketAddrV4;

use bytes::Bytes;
use tokio::net::UnixStream;

use crate::frame::{IpcServiceFrame, Pf, PACKET_FILTER_SERVICE};
use crate::utils::write;

/// Client to update dynamic list of packet filter rules.
#[derive(Default, Debug)]
pub struct PacketFilter {
    inner: Option<UnixStream>,
}

impl PacketFilter {
    pub fn new() -> Self {
        PacketFilter::default()
    }

    // Todo: remove this method.
    pub fn init(&mut self, stream: UnixStream) {
        self.inner = Some(stream);
    }

    pub async fn connect(&mut self) -> io::Result<()> {
        let sock = self.inner.as_ref().expect("To be initialized");
        let service = PACKET_FILTER_SERVICE.to_le_bytes();
        write(sock, Bytes::from(service.to_vec())).await
    }

    pub async fn add(&self, addr: SocketAddrV4) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = IpcServiceFrame::Pf(Pf { op: Pf::ADD, addr });
            write(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn remove(&self, addr: SocketAddrV4) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = IpcServiceFrame::Pf(Pf {
                op: Pf::REMOVE,
                addr,
            });
            write(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }
}
