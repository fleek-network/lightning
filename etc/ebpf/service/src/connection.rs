use std::borrow::BorrowMut;
use std::sync::Arc;

use anyhow::bail;
use aya::maps::{HashMap, MapData};
use bytes::Bytes;
use common::IpPortKey;
use tokio::io::Interest;
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use crate::schema::Pf;

pub struct Connection<T> {
    socket: UnixStream,
    block_list: Arc<Mutex<HashMap<T, IpPortKey, u32>>>,
}

impl<T: BorrowMut<MapData>> Connection<T> {
    pub fn new(socket: UnixStream, block_list: Arc<Mutex<HashMap<T, IpPortKey, u32>>>) -> Self {
        Self { socket, block_list }
    }

    async fn handle_request(&mut self, message: Pf) -> anyhow::Result<()> {
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

    pub async fn handle(mut self) -> anyhow::Result<()> {
        loop {
            let ready = self.socket.ready(Interest::READABLE).await?;
            if ready.is_readable() {
                let mut read_buf = vec![0u8; 7];
                let mut bytes_read = 0;
                'read: loop {
                    while bytes_read < 7 {
                        match self.socket.try_read(&mut read_buf) {
                            Ok(0) => {
                                return Ok(());
                            },
                            Ok(n) => {
                                bytes_read += n;
                            },
                            Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                                // We received a false positive.
                                continue;
                            },
                            Err(e) => {
                                return Err(e.into());
                            },
                        }
                    }

                    let message = Pf::try_from(Bytes::from(read_buf.clone()))?;
                    self.handle_request(message).await?;
                    break 'read;
                }
            }
        }
    }
}
