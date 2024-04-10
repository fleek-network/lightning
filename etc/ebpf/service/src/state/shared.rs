use std::collections::HashSet;
use std::net::SocketAddrV4;
use std::sync::Arc;

use anyhow::bail;
use aya::maps::{HashMap, MapData};
use common::{File, PacketFilter};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::state::rules::PacketFilterRule;

const EBPF_STATE_TMP_DIR: &str = "~/.lightning/ebpf/state/tmp";
const EBPF_STATE_PF_DIR: &str = "~/.lightning/ebpf/state/packet-filter";
const RULES_FILE: &str = "filters.json";

#[derive(Clone)]
pub struct SharedState {
    packet_filters: Arc<Mutex<HashMap<MapData, PacketFilter, u32>>>,
    file_open_allow_binfile: Arc<Mutex<HashMap<MapData, File, u64>>>,
    file_open_deny_binfile: Arc<Mutex<HashMap<MapData, File, u64>>>,
}

impl SharedState {
    pub fn new(
        packet_filters: HashMap<MapData, PacketFilter, u32>,
        file_open_allow_binfile: HashMap<MapData, File, u64>,
        file_open_deny_binfile: HashMap<MapData, File, u64>,
    ) -> Self {
        Self {
            packet_filters: Arc::new(Mutex::new(packet_filters)),
            file_open_allow_binfile: Arc::new(Mutex::new(file_open_allow_binfile)),
            file_open_deny_binfile: Arc::new(Mutex::new(file_open_deny_binfile)),
        }
    }

    pub async fn packet_filter_add(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.packet_filters.lock().await;
        map.insert(
            PacketFilter {
                ip: (*addr.ip()).into(),
                port: addr.port() as u32,
            },
            0,
            0,
        )?;
        Ok(())
    }

    pub async fn packet_filter_remove(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.packet_filters.lock().await;
        map.remove(&PacketFilter {
            ip: (*addr.ip()).into(),
            port: addr.port() as u32,
        })?;
        Ok(())
    }

    pub async fn file_open_allow(&mut self, inode: u64, dev: u32, rdev: u32) -> anyhow::Result<()> {
        let mut map = self.file_open_allow_binfile.lock().await;
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

    pub async fn file_open_deny(&mut self, inode: u64, dev: u32, rdev: u32) -> anyhow::Result<()> {
        let mut map = self.file_open_deny_binfile.lock().await;
        map.remove(&File {
            inode_n: inode,
            dev,
            rdev,
        })?;
        Ok(())
    }

    pub async fn pull_packet_filters_from_config(&self) -> anyhow::Result<()> {
        let mut file = fs::File::open(format!("{EBPF_STATE_PF_DIR}/{RULES_FILE}")).await?;
        let mut buf = String::new();
        file.read_to_string(&mut buf).await?;
        let filters: Vec<PacketFilterRule> = serde_json::from_str(&buf)?;
        let new_state = filters
            .into_iter()
            .map(Into::into)
            .collect::<HashSet<PacketFilter>>();

        let mut map = self.packet_filters.lock().await;
        // Due to a constraint of the aya api, there is no clean method for the maps and
        // we don't get mutable access as iterator is read only.
        let mut remove = Vec::new();
        for filter in map.keys() {
            let f = filter?;
            if !new_state.contains(&f) {
                remove.push(f);
            }
        }

        for filter in new_state {
            map.insert(PacketFilter::from(filter), 0, 0)?;
        }

        for filter in remove {
            map.remove(&PacketFilter::from(filter))?;
        }

        Ok(())
    }

    pub async fn sync_packet_filters(&self) -> anyhow::Result<()> {
        let tmp_path = format!("{EBPF_STATE_TMP_DIR}/{RULES_FILE}");
        let mut tmp = fs::File::create(tmp_path.clone()).await?;
        let mut filters = Vec::new();
        let guard = self.packet_filters.lock().await;
        for key in guard.keys() {
            match key {
                Ok(key) => {
                    filters.push(PacketFilterRule {
                        ip: key.ip.into(),
                        port: key.port as u16,
                    });
                },
                Err(e) => bail!("failed to sync packet filters: {e:?}"),
            }
        }

        let bytes = serde_json::to_string(&filters)?;
        tmp.write_all(bytes.as_bytes()).await?;

        tmp.sync_all().await?;

        fs::rename(tmp_path, format!("{EBPF_STATE_PF_DIR}/{RULES_FILE}")).await?;

        Ok(())
    }
}
