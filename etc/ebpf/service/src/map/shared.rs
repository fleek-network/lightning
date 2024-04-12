use std::net::SocketAddrV4;
use std::sync::Arc;

use aya::maps::{HashMap, MapData};
use common::{File, PacketFilter, PacketFilterParams};
use tokio::sync::Mutex;

use crate::config::ConfigSource;
use crate::map::{FileOpenRule, PacketFilterRule, PermissionPolicy};

#[derive(Clone)]
pub struct SharedMap {
    packet_filters: Arc<Mutex<HashMap<MapData, PacketFilter, PacketFilterParams>>>,
    file_open_rules: Arc<Mutex<FileOpenMaps>>,
    storage: ConfigSource,
}

impl SharedMap {
    pub fn new(
        packet_filters: HashMap<MapData, PacketFilter, PacketFilterParams>,
        file_open_allow_rules: HashMap<MapData, File, u64>,
        file_open_deny_rules: HashMap<MapData, File, u64>,
    ) -> Self {
        Self {
            packet_filters: Arc::new(Mutex::new(packet_filters)),
            file_open_rules: Arc::new(Mutex::new(FileOpenMaps {
                file_open_allow_rules,
                file_open_deny_rules,
            })),
            storage: Default::default(),
        }
    }

    pub async fn packet_filter_add(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.packet_filters.lock().await;
        map.insert(
            PacketFilter {
                ip: u32::from_be_bytes(addr.ip().octets()),
                port: addr.port(),
                proto: PacketFilterRule::TCP,
            },
            PacketFilterParams {
                trigger_event: 1,
                shortlived: 1,
                action: PacketFilterRule::DROP,
            },
            0,
        )?;
        Ok(())
    }

    pub async fn packet_filter_remove(&mut self, addr: SocketAddrV4) -> anyhow::Result<()> {
        let mut map = self.packet_filters.lock().await;
        map.remove(&PacketFilter {
            ip: u32::from_be_bytes(addr.ip().octets()),
            port: addr.port(),
            proto: PacketFilterRule::TCP,
        })?;
        Ok(())
    }

    /// Updates packet filters.
    ///
    /// Reads from disk so it's a heavy operation.
    pub async fn update_packet_filters(&self) -> anyhow::Result<()> {
        let filters: Vec<PacketFilterRule> = self.storage.read_packet_filters().await?;
        let new_state = filters
            .into_iter()
            .map(|filter| (PacketFilter::from(filter), PacketFilterParams::from(filter)))
            .collect::<std::collections::HashMap<_, _>>();

        let mut map = self.packet_filters.lock().await;
        // Due to a constraint of the aya api, there is no clean method for the maps and
        // we don't get mutable access as iterator is read only.
        let mut remove = Vec::new();
        for result in map.iter() {
            let (filter, params) = result?;
            // Filters with shortlived=1 do not get removed.
            // This is to support dynamic ephemiral rules
            // that may be produced by rate limiting, for example.
            if !new_state.contains_key(&filter) && params.shortlived != 1 {
                remove.push(filter);
            }
        }

        for (filter, params) in new_state {
            map.insert(filter, params, 0)?;
        }

        for filter in remove {
            map.remove(&filter)?;
        }

        Ok(())
    }

    /// Updates file-open rules.
    ///
    /// Reads from disk so it's a heavy operation.
    pub async fn update_file_open_rules(&self) -> anyhow::Result<()> {
        let rules: Vec<FileOpenRule> = self.storage.read_profiles().await?;

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
}

pub struct FileOpenMaps {
    file_open_allow_rules: HashMap<MapData, File, u64>,
    file_open_deny_rules: HashMap<MapData, File, u64>,
}
