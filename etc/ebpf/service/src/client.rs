use std::io;
use std::net::SocketAddrV4;

use bytes::Bytes;
use tokio::io::Interest;
use tokio::net::UnixStream;

use crate::schema::{EbpfServiceFrame, FileOpen, FileOpenSrc, Pf};

#[derive(Default, Debug)]
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

    pub async fn blocklist_add(&self, addr: SocketAddrV4) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::Pf(Pf { op: Pf::ADD, addr });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn blocklist_remove(&self, addr: SocketAddrV4) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::Pf(Pf {
                op: Pf::REMOVE,
                addr,
            });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn file_open_allow_proc(&self, pid: u64) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::FileOpen(FileOpen {
                op: FileOpen::ALLOW,
                src: FileOpenSrc::Pid(pid),
            });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn file_open_block_proc(&self, pid: u64) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::FileOpen(FileOpen {
                op: FileOpen::BLOCK,
                src: FileOpenSrc::Pid(pid),
            });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn file_open_allow_bin(&self, inode: u64, dev: u32, rdev: u32) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::FileOpen(FileOpen {
                op: FileOpen::ALLOW,
                src: FileOpenSrc::Bin { inode, dev, rdev },
            });
            Self::send(stream, frame.serialize_len_delimit()).await?;
        }
        Ok(())
    }

    pub async fn file_open_block_bin(&self, inode: u64, dev: u32, rdev: u32) -> io::Result<()> {
        if let Some(stream) = &self.inner {
            let frame = EbpfServiceFrame::FileOpen(FileOpen {
                op: FileOpen::BLOCK,
                src: FileOpenSrc::Bin { inode, dev, rdev },
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
