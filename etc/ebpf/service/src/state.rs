use core::borrow::BorrowMut;
use std::net::SocketAddrV4;
use std::sync::Arc;

use aya::maps::{HashMap, MapData};
use common::IpPortKey;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct SharedState<B> {
    block_list: Arc<Mutex<HashMap<B, IpPortKey, u32>>>,
}

impl<B> SharedState<B>
where
    B: BorrowMut<MapData>,
{
    pub fn new(block_list: Arc<Mutex<HashMap<B, IpPortKey, u32>>>) -> Self {
        Self { block_list }
    }

    pub async fn blocklist_add(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.block_list.lock().await;
        map.insert(
            IpPortKey {
                ip: (*addr.ip()).into(),
                port: addr.port() as u32,
            },
            0,
            0,
        )?;
        Ok(())
    }

    pub async fn blocklist_remove(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.block_list.lock().await;
        map.remove(&IpPortKey {
            ip: (*addr.ip()).into(),
            port: addr.port() as u32,
        })?;
        Ok(())
    }
}
