use std::net::Ipv4Addr;

use common::PacketFilter;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct FilterForStorage {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl From<FilterForStorage> for PacketFilter {
    fn from(value: FilterForStorage) -> Self {
        Self {
            ip: u32::from_be_bytes(value.ip.octets()),
            port: value.port as u32,
        }
    }
}
