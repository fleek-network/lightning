use std::collections::HashSet;
use std::net::SocketAddrV4;
use std::sync::Arc;

use anyhow::bail;
use aya::maps::{HashMap, MapData};
use common::{File, PacketFilter};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::state::rules::{FileOpenRule, PacketFilterRule, PermissionPolicy};

const EBPF_STATE_TMP_DIR: &str = "~/.lightning/ebpf/state/tmp";
const EBPF_STATE_PF_DIR: &str = "~/.lightning/ebpf/state/packet-filter";
const RULES_FILE: &str = "filters.json";

#[derive(Clone)]
pub struct SharedState {
    packet_filters: Arc<Mutex<HashMap<MapData, PacketFilter, u32>>>,
    file_open_rules: Arc<Mutex<FileOpenMaps>>,
}

impl SharedState {
    pub fn new(
        packet_filters: HashMap<MapData, PacketFilter, u32>,
        file_open_allow_rules: HashMap<MapData, File, u64>,
        file_open_deny_rules: HashMap<MapData, File, u64>,
    ) -> Self {
        Self {
            packet_filters: Arc::new(Mutex::new(packet_filters)),
            file_open_rules: Arc::new(Mutex::new(FileOpenMaps {
                file_open_allow_rules,
                file_open_deny_rules,
            })),
        }
    }

    pub async fn packet_filter_add(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.packet_filters.lock().await;
        map.insert(
            PacketFilter {
                ip: (*addr.ip()).into(),
                port: addr.port() as u32,
            },
            1,
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

    pub async fn update_packet_filters(&self) -> anyhow::Result<()> {
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
        for result in map.iter() {
            let (filter, flag) = result?;
            // Filters with flag=1 do not get removed.
            // This is to support dynamic ephemiral rules
            // that may be produced by rate limiting, for example.
            if !new_state.contains(&filter) && flag != 1 {
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

    pub async fn update_file_open_rules(&self) -> anyhow::Result<()> {
        let mut file = fs::File::open(format!("{EBPF_STATE_PF_DIR}/{RULES_FILE}")).await?;
        let mut buf = String::new();
        file.read_to_string(&mut buf).await?;
        let rules: Vec<FileOpenRule> = serde_json::from_str(&buf)?;

        let mut allow = Vec::new();
        let mut deny = Vec::new();

        for rule in rules {
            if let PermissionPolicy::Allow = &rule.permission {
                allow.push(rule);
            } else {
                deny.push(rule);
            }
        }

        let mut maps = self.file_open_rules.lock().await;

        // Due to a constraint of the aya api, there is no clean method for the maps
        // so we remove all of them. Todo: Let's open an issue with aya.
        let mut remove = Vec::new();
        for rule in maps.file_open_deny_rules.keys() {
            remove.push(rule);
        }
        for rule in remove {
            let r = rule?;
            maps.file_open_deny_rules.remove(&File {
                inode_n: r.inode_n,
                dev: r.dev,
                rdev: r.rdev,
            })?;
        }

        let mut remove = Vec::new();
        for rule in maps.file_open_allow_rules.keys() {
            remove.push(rule);
        }
        for rule in remove {
            let r = rule?;
            maps.file_open_allow_rules.remove(&File {
                inode_n: r.inode_n,
                dev: r.dev,
                rdev: r.rdev,
            })?;
        }

        for rule in deny {
            maps.file_open_deny_rules.insert(File::from(rule), 0, 0)?;
        }

        for rule in allow {
            maps.file_open_allow_rules.insert(File::from(rule), 0, 0)?;
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

pub struct FileOpenMaps {
    file_open_allow_rules: HashMap<MapData, File, u64>,
    file_open_deny_rules: HashMap<MapData, File, u64>,
}
