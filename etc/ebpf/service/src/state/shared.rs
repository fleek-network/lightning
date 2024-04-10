use std::net::SocketAddrV4;
use std::sync::Arc;

use anyhow::bail;
use aya::maps::{HashMap, MapData};
use common::{File, IpPortKey};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::state::schema::Filter;

const EBPF_STATE_TMP_DIR: &str = "~/.lightning/ebpf/state/tmp";
const EBPF_STATE_PF_DIR: &str = "~/.lightning/ebpf/state/packet-filter";
const RULES_FILE: &str = "rules.json";

#[derive(Clone)]
pub struct SharedState {
    blocklist: Arc<Mutex<HashMap<MapData, IpPortKey, u32>>>,
    proc_to_file: Arc<Mutex<HashMap<MapData, u64, u64>>>,
    binfile_to_file: Arc<Mutex<HashMap<MapData, File, u64>>>,
}

impl SharedState {
    pub fn new(
        block_list: HashMap<MapData, IpPortKey, u32>,
        proc_to_file: HashMap<MapData, u64, u64>,
        binfile_to_file: HashMap<MapData, File, u64>,
    ) -> Self {
        Self {
            blocklist: Arc::new(Mutex::new(block_list)),
            proc_to_file: Arc::new(Mutex::new(proc_to_file)),
            binfile_to_file: Arc::new(Mutex::new(binfile_to_file)),
        }
    }

    pub async fn blocklist_add(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.blocklist.lock().await;
        map.insert(
            IpPortKey {
                ip: (*addr.ip()).into(),
                port: addr.port() as u32,
            },
            0,
            0,
        )?;
        self.sync_blocklist().await
    }

    pub async fn blocklist_remove(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.blocklist.lock().await;
        map.remove(&IpPortKey {
            ip: (*addr.ip()).into(),
            port: addr.port() as u32,
        })?;
        self.sync_blocklist().await
    }

    pub async fn file_allow_open_for_proc(&mut self, pid: u64) -> anyhow::Result<()> {
        let mut map = self.proc_to_file.lock().await;
        map.insert(pid, 0, 0)?;
        Ok(())
    }

    pub async fn file_block_open_for_proc(&mut self, pid: u64) -> anyhow::Result<()> {
        let mut map = self.proc_to_file.lock().await;
        map.remove(&pid)?;
        Ok(())
    }

    pub async fn file_allow_open_for_binf(
        &mut self,
        inode: u64,
        dev: u32,
        rdev: u32,
    ) -> anyhow::Result<()> {
        let mut map = self.binfile_to_file.lock().await;
        map.insert(
            File {
                inode_n: inode,
                dev,
                rdev,
            },
            0,
            0,
        )?;
        Ok(())
    }

    pub async fn file_block_open_for_binf(
        &mut self,
        inode: u64,
        dev: u32,
        rdev: u32,
    ) -> anyhow::Result<()> {
        let mut map = self.binfile_to_file.lock().await;
        map.remove(&File {
            inode_n: inode,
            dev,
            rdev,
        })?;
        Ok(())
    }

    async fn sync_blocklist(&self) -> anyhow::Result<()> {
        let tmp_path = format!("{EBPF_STATE_TMP_DIR}/{RULES_FILE}");
        let mut tmp = fs::File::create(tmp_path.clone()).await?;
        let mut rules = Vec::new();
        let guard = self.blocklist.lock().await;
        for key in guard.keys() {
            match key {
                Ok(key) => {
                    rules.push(Filter {
                        ip: key.ip.into(),
                        port: key.port as u16,
                    });
                },
                Err(e) => bail!("failed to sync blocklist: {e:?}"),
            }
        }

        let bytes = serde_json::to_string(&rules)?;
        tmp.write_all(bytes.as_bytes()).await?;

        tmp.sync_all().await?;

        fs::rename(tmp_path, format!("{EBPF_STATE_PF_DIR}/{RULES_FILE}")).await?;

        Ok(())
    }
}
