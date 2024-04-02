use std::net::SocketAddrV4;
use std::sync::Arc;

use aya::maps::{HashMap, MapData};
use common::IpPortKey;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct SharedState {
    block_list: Arc<Mutex<HashMap<MapData, IpPortKey, u32>>>,
    proc_to_file: Arc<Mutex<HashMap<MapData, u64, u64>>>,
}

impl SharedState {
    pub fn new(
        block_list: HashMap<MapData, IpPortKey, u32>,
        proc_to_file: HashMap<MapData, u64, u64>,
    ) -> Self {
        Self {
            block_list: Arc::new(Mutex::new(block_list)),
            proc_to_file: Arc::new(Mutex::new(proc_to_file)),
        }
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

    pub async fn file_allow_open(&mut self, pid: u64) -> anyhow::Result<()> {
        let mut map = self.proc_to_file.lock().await;
        map.insert(pid, 0, 0)?;
        Ok(())
    }

    pub async fn file_block_open(&mut self, pid: u64) -> anyhow::Result<()> {
        let mut map = self.proc_to_file.lock().await;
        map.remove(&pid)?;
        Ok(())
    }
}
