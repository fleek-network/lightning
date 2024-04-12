use std::net::Ipv4Addr;

#[cfg(feature = "server")]
use common::{File, PacketFilter, PacketFilterParams};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct PacketFilterRule {
    /// Subnet prefix.
    pub prefix: u32,
    /// Source IP address.
    pub ip: Ipv4Addr,
    /// Source port.
    pub port: u16,
    /// Transport protocol.
    ///
    /// Uses values from Ipv4 header.
    pub proto: u16,
    /// Flag set to true when we should trigger
    /// an event from kernel space.
    pub trigger_event: bool,
    /// Flag set to true if this is a short-lived filter.
    ///
    /// Short-lived filters do not get saved in storage.
    #[serde(skip)]
    pub shortlived: bool,
    /// Action to take e.g. DROP or PASS.
    pub action: u32,
}

impl PacketFilterRule {
    pub const DROP: u32 = 1;
    pub const PASS: u32 = 2;
    pub const TCP: u16 = 6;
    pub const UDP: u16 = 17;
    pub const DEFAULT_PREFIX: u32 = 32;
}

impl PacketFilterRule {
    pub fn action_str(&self) -> String {
        match self.action {
            Self::DROP => "drop".to_string(),
            Self::PASS => "pass".to_string(),
            _ => "N/A".to_string(),
        }
    }

    pub fn proto_str(&self) -> String {
        match self.proto {
            Self::TCP => "tcp".to_string(),
            Self::UDP => "udp".to_string(),
            _ => "N/A".to_string(),
        }
    }
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
        PacketFilter {
            ip: u32::from_be_bytes(value.ip.octets()),
            port: value.port,
            proto: value.proto,
        }
    }
}

#[cfg(feature = "server")]
impl From<PacketFilterRule> for PacketFilterParams {
    fn from(value: PacketFilterRule) -> Self {
        PacketFilterParams {
            trigger_event: value.trigger_event as u16,
            shortlived: value.shortlived as u16,
            action: value.action,
        }
    }
}
