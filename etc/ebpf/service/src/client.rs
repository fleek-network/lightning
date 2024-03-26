use std::io;
use std::net::SocketAddrV4;

use bytes::Bytes;
use log::error;
use tokio::io::Interest;
use tokio::net::UnixStream;

use crate::schema::{EbpfServiceFrame, Pf};

#[derive(Default)]
pub struct EbpfSvcClient {
    inner: Option<UnixStream>,
}

impl EbpfSvcClient {
    pub fn new() -> Self {
        EbpfSvcClient::default()
    }

    pub fn init(&mut self, stream: UnixStream) {
        self.inner = Some(stream);
    }

    pub async fn add_to_block_list(&self, addr: SocketAddrV4) {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::Pf(Pf { op: Pf::ADD, addr });
            if let Err(e) = Self::send(stream, frame.serialize_len_delimit()).await {
                error!("failed to add to the block-list: {e:?}");
            }
        }
    }

    pub async fn remove_from_block_list(&self, addr: SocketAddrV4) {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::Pf(Pf {
                op: Pf::REMOVE,
                addr,
            });
            if let Err(e) = Self::send(stream, frame.serialize_len_delimit()).await {
                error!("failed to remove from the block-list: {e:?}");
            }
        }
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
