use std::net::Ipv4Addr;

use common::PacketFilter;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct PacketFilterRule {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl From<PacketFilterRule> for PacketFilter {
    fn from(value: PacketFilterRule) -> Self {
        Self {
            ip: u32::from_be_bytes(value.ip.octets()),
            port: value.port as u32,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct FileOpenRule {
    pub policy: Policy,
    pub params: FileOpenParams,
}

#[derive(Deserialize, Serialize)]
pub enum Policy {
    Allow,
    Deny,
}

#[derive(Deserialize, Serialize)]
pub enum FileOpenParams {
    Pid(u64),
    Bin { inode: u64, dev: u32, rdev: u32 },
}
