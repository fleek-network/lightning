use std::net::Ipv4Addr;

#[cfg(feature = "server")]
use common::{PacketFilter, PacketFilterParams};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

/// Lightning Packet Filter rule.
///
/// The filter rule specifies the action to perform
/// on incoming IP network packets when they match
/// the IP address, port, subnet and protocol specified
/// in this filter.
#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct PacketFilterRule {
    /// Subnet prefix.
    pub prefix: u32,
    /// Source IP address.
    pub ip: Ipv4Addr,
    /// Source port.
    ///
    /// Wildcard value is `0`.
    pub port: u16,
    /// Transport protocol.
    ///
    /// Uses values from Ipv4 header.
    /// Wildcard value is [`u16::MAX`].
    pub proto: u16,
    /// Audit mode.
    ///
    /// If set to true, logs on match.
    /// Defaults to false.
    pub audit: bool,
    /// Flag set to true if this is a short-lived filter.
    ///
    /// Short-lived filters do not get saved in storage.
    #[serde(skip)]
    pub shortlived: bool,
    /// Action to perform e.g. DROP or PASS.
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
            trigger_event: value.audit as u16,
            shortlived: value.shortlived as u16,
            action: value.action,
        }
    }
}

/// Lightning Security Profile.
///
/// A profile specifies a list of files that a program
/// can access and the operations the program may perform.
#[derive(Deserialize, Serialize)]
pub struct Profile {
    /// Path to the executable file.
    ///
    /// If `None`, the profile will apply to all processes.
    pub name: Option<ResolvedPathBuf>,
    /// File rules.
    ///
    /// These control how files are accessed by the
    /// executable (or process if a name was not provided).
    /// Please see [`FileRule`].
    pub file_rules: Vec<FileRule>,
    /// Audit mode.
    ///
    /// If set to true, logs every access decision.
    /// Normally, only access decisions made without
    /// a cooresponding rule will be logged.
    pub audit: bool,
}

/// Rule that defines how a file is accessed.
#[derive(Deserialize, Serialize)]
pub struct FileRule {
    /// Path of the file.
    pub file: ResolvedPathBuf,
    /// Operations.
    pub operations: u8,
    /// If set to true, allows the operation(s).
    /// Otherwise, it denies the operation(s).
    pub allow: bool,
}

impl FileRule {
    pub const OPEN_MASK: u8 = 0x01 << 0;
    pub const READ_MASK: u8 = 0x01 << 1;
    pub const WRITE_MASK: u8 = 0x01 << 2;
    pub const EXEC_MASK: u8 = 0x01 << 3;
}
