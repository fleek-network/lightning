use std::io;
use std::net::SocketAddrV4;

use bytes::Bytes;
use tokio::io::Interest;
use tokio::net::UnixStream;

use crate::frame::{IpcServiceFrame, Pf};

#[derive(Default, Debug)]
pub struct IpcClient {
    inner: Option<UnixStream>,
}

impl IpcClient {
    pub fn new() -> Self {
        IpcClient::default()
    }

    pub fn init(&mut self, stream: UnixStream) {
        self.inner = Some(stream);
    }

    pub async fn packet_filter_add(&self, addr: SocketAddrV4) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = IpcServiceFrame::Pf(Pf { op: Pf::ADD, addr });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn packet_filter_remove(&self, addr: SocketAddrV4) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = IpcServiceFrame::Pf(Pf {
                op: Pf::REMOVE,
                addr,
            });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    async fn send(stream: &UnixStream, bytes: Bytes) -> io::Result<()> {
        let mut bytes_to_write = 0;
        loop {
            stream.ready(Interest::WRITABLE).await?;
            'write: while bytes_to_write < bytes.len() {
                match stream.try_write(&bytes[bytes_to_write..]) {
                    Ok(n) => {
                        bytes_to_write += n;
                    },
                    Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        // We received a false positive.
                        break 'write;
                    },
                    Err(e) => {
                        return Err(e.into());
                    },
                }
            }
        }
    }
}
