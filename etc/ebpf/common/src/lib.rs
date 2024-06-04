#![no_std]

pub const MAX_DEVICES: usize = 2;
pub const MAX_FILE_RULES: usize = 10;
pub const ALLOW_FILE_RULE: i32 = 0;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct PacketFilter {
    /// Source IPv4 address.
    pub ip: u32,
    /// Source port.
    pub port: u16,
    /// Transport protocol.
    ///
    /// Uses values from Ipv4 header.
    /// Use `u16::MAX` to indicate `any`.
    pub proto: u16,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for PacketFilter {}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct PacketFilterParams {
    /// Flag set to true=1 when we should trigger
    /// an event from kernel space.
    pub trigger_event: u16,
    /// Flag set to true=1 if this is a short-lived filter.
    ///
    /// Short-lived filters do not get saved in storage.
    pub shortlived: u16,
    /// Action to take.
    ///
    /// XDP_ABORTED  = 0;
    /// XDP_DROP     = 1;
    /// XDP_PASS     = 2;
    /// XDP_TX       = 3;
    /// XDP_REDIRECT = 4;
    pub action: u32,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for PacketFilterParams {}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct SubnetFilterParams {
    /// Source port.
    pub port: u16,
    /// Transport protocol.
    ///
    /// Uses values from Ipv4 header.
    /// Use `u16::MAX` to indicate `any`.
    pub proto: u16,
    /// Extra parameters.
    pub extra: PacketFilterParams,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for SubnetFilterParams {}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FileRuleList {
    /// The files that are being protected.
    pub rules: [FileRule; MAX_FILE_RULES],
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for FileRuleList {}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct File {
    /// Inode ID of the file.
    pub inode: u64,
    /// The device this file is located on.
    pub dev: u32,
}

impl File {
    pub fn new(inode: u64) -> Self {
        Self {
            inode,
            // Todo: This is not supported yet.
            dev: 0,
        }
    }
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for File {}

#[derive(Clone, Copy, Debug)]
pub struct FileRule {
    /// The file in question.
    pub inode: u64,
    /// Flag set to false=-1 if the operation
    /// in question is not allowed for these files,
    /// otherwise set to true=0.
    pub allow: i32,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for FileRule {}

impl Default for FileRule {
    fn default() -> Self {
        Self {
            inode: 0,
            allow: -1,
        }
    }
}
