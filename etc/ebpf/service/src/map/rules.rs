use std::net::Ipv4Addr;

#[cfg(feature = "server")]
use common::{File, PacketFilter};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct PacketFilterRule {
    pub ip: Ipv4Addr,
    pub port: u16,
}

#[derive(Deserialize, Serialize)]
pub struct FileOpenRule {
    pub permission: PermissionPolicy,
    pub inode: u64,
    pub dev: u32,
    pub rdev: u32,
}

#[derive(Deserialize, Serialize)]
pub enum PermissionPolicy {
    Allow,
    Deny,
}

#[cfg(feature = "server")]
impl From<FileOpenRule> for File {
    fn from(value: FileOpenRule) -> Self {
        Self {
            inode_n: value.inode,
            dev: value.dev,
            rdev: value.rdev,
        }
    }
}

#[cfg(feature = "server")]
impl From<PacketFilterRule> for PacketFilter {
    fn from(value: PacketFilterRule) -> Self {
        Self {
            ip: u32::from_be_bytes(value.ip.octets()),
            port: value.port as u32,
        }
    }
}
