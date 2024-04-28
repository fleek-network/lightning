use std::net::SocketAddrV4;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, bail};
use aya::maps::{HashMap, MapData};
use common::{
    File,
    FileRuleList,
    PacketFilter,
    PacketFilterParams,
    ALLOW_FILE_RULE,
    MAX_FILE_RULES,
};
use tokio::fs;
use tokio::sync::Mutex;

use crate::config::ConfigSource;
use crate::map::{FileRule, PacketFilterRule};

#[derive(Clone)]
pub struct SharedMap {
    packet_filters: Arc<Mutex<HashMap<MapData, PacketFilter, PacketFilterParams>>>,
    file_open_rules: Arc<Mutex<HashMap<MapData, File, FileRuleList>>>,
    storage: ConfigSource,
}

impl SharedMap {
    pub fn new(
        packet_filters: HashMap<MapData, PacketFilter, PacketFilterParams>,
        file_open_rules: HashMap<MapData, File, FileRuleList>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            packet_filters: Arc::new(Mutex::new(packet_filters)),
            file_open_rules: Arc::new(Mutex::new(file_open_rules)),
            storage: ConfigSource::create_config()?,
        })
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
    pub async fn update_file_rules(&self) -> anyhow::Result<()> {
        let profiles = self.storage.get_profiles().await?;

        let mut new = std::collections::HashMap::new();
        for profile in profiles {
            let exec = file_from_path(profile.name.as_ref().unwrap()).await?;
            let mut file_open_rules = vec![common::FileRule::default(); MAX_FILE_RULES];
            for (i, rule) in profile.file_rules.iter().enumerate() {
                // Todo: check for other types of accesses.
                if rule.operations == FileRule::OPEN_MASK {
                    let file = file_from_path(&rule.file).await?;
                    if exec.dev != file.dev {
                        // Protecting files in more than one device is not supported yet.
                        bail!("executable file device and file device do not match");
                    }
                    if i >= MAX_FILE_RULES {
                        bail!("path maximum {MAX_FILE_RULES} execeeded");
                    }
                    file_open_rules[i].inode = file.inode;
                    file_open_rules[i].allow = ALLOW_FILE_RULE;
                }
            }

            let rules: [common::FileRule; MAX_FILE_RULES] =
                file_open_rules.try_into().map_err(|_| anyhow!("eerror"))?;
            new.insert(exec, FileRuleList { rules });
        }

        let mut maps = self.file_open_rules.lock().await;

        // Due to a constraint of the aya api, there is no clean method for the maps
        // so we remove all of them. Todo: Let's open an issue with aya.
        let mut remove = Vec::new();
        for file in maps.keys() {
            remove.push(file);
        }
        for rule in remove {
            let r = rule?;
            maps.remove(&r)?;
        }

        for (exec, rules) in new {
            maps.insert(exec, rules, 0)?;
        }

        Ok(())
    }
}

async fn file_from_path(path: &PathBuf) -> anyhow::Result<File> {
    let file = fs::File::open(path.as_path()).await?;
    let metadata = file.metadata().await?;
    let inode = metadata.ino();
    let dev = metadata.dev();
    Ok(File {
        inode,
        dev: dev.try_into()?,
    })
}
