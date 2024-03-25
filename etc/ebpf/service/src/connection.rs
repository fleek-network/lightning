use std::borrow::BorrowMut;
use std::convert::Infallible;
use std::sync::Arc;

use anyhow::bail;
use aya::maps::{HashMap, MapData};
use bytes::Bytes;
use common::IpPortKey;
use log::error;
use tokio::io::Interest;
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use crate::schema::{EbpfServiceFrame, Pf};

pub struct Connection<T> {
    socket: UnixStream,
    block_list: Arc<Mutex<HashMap<T, IpPortKey, u32>>>,
}

impl<T: BorrowMut<MapData>> Connection<T> {
    pub fn new(socket: UnixStream, block_list: Arc<Mutex<HashMap<T, IpPortKey, u32>>>) -> Self {
        Self { socket, block_list }
    }

    #[inline]
    async fn pf_handle(&mut self, message: Pf) -> anyhow::Result<()> {
        match message.op {
            Pf::ADD => {
                let mut map = self.block_list.lock().await;
                map.insert(
                    IpPortKey {
                        ip: (*message.addr.ip()).into(),
                        port: message.addr.port() as u32,
                    },
                    0,
                    0,
                )?;
            },
            Pf::REMOVE => {
                let mut map = self.block_list.lock().await;
                map.remove(&IpPortKey {
                    ip: (*message.addr.ip()).into(),
                    port: message.addr.port() as u32,
                })?;
            },
            op => {
                bail!("invalid op: {op:?}");
            },
        }
        Ok(())
    }

    #[inline]
    async fn handle_request(&mut self, frame: EbpfServiceFrame) -> anyhow::Result<()> {
        match frame {
            EbpfServiceFrame::Mac => unimplemented!(),
            EbpfServiceFrame::Pf(pf) => self.pf_handle(pf).await,
        }
    }

    pub async fn handle(mut self) -> anyhow::Result<()> {
        let mut read_buf = vec![0u8; 8];
        let mut bytes_read = 0;
        let mut frame_len = 0;
        loop {
            let ready = self.socket.ready(Interest::READABLE).await?;
            if ready.is_readable() {
                'read: loop {
                    while frame_len == 0 && bytes_read < 8 {
                        match self.socket.try_read(&mut read_buf[bytes_read..]) {
                            Ok(0) => {
                                return Ok(());
                            },
                            Ok(n) => {
                                bytes_read += n;
                            },
                            Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                                // We received a false positive.
                                break 'read;
                            },
                            Err(e) => {
                                return Err(e.into());
                            },
                        }
                    }

                    if frame_len == 0 {
                        let bytes: [u8; 8] = read_buf.as_slice().try_into()?;
                        frame_len = usize::from_be_bytes(bytes);
                        // We subtract here to pass entire buffer
                        // to EbpfServiceFrame deserializer further below.
                        read_buf.resize(frame_len - 8, 0);
                        bytes_read = 0;
                    }

                    while bytes_read < frame_len {
                        match self.socket.try_read(&mut read_buf[bytes_read..]) {
                            Ok(0) => {
                                return Ok(());
                            },
                            Ok(n) => {
                                bytes_read += n;
                            },
                            Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                                // We received a false positive.
                                break 'read;
                            },
                            Err(e) => {
                                return Err(e.into());
                            },
                        }
                    }

                    match EbpfServiceFrame::try_from(read_buf.as_slice()) {
                        Ok(f) => {
                            if let Err(e) = self.handle_request(f).await {
                                error!("failed to handle request: {e:?}");
                            }
                        },
                        Err(e) => {
                            error!("failed to deserialize frame: {e:?}");
                        },
                    }

                    read_buf.resize(8, 0);
                    bytes_read = 0;
                    frame_len = 0;

                    break 'read;
                }
            }
        }
    }
}
