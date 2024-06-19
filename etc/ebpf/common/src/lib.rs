#![no_std]

pub const MAX_DEVICES: usize = 2;
pub const MAX_FILE_RULES: usize = 20;
pub const EVENT_HEADER_SIZE: usize = 2;
pub const MAX_BUFFER_LEN: usize = 1024;
pub const FILE_OPEN_PROG_ID: u8 = 0;
pub const ACCESS_DENIED_EVENT: u8 = 0;

pub type Buffer = [u8; MAX_BUFFER_LEN];
pub type EventMessage = [u8; EVENT_HEADER_SIZE + 2 * MAX_BUFFER_LEN];

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

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Profile {
    /// The files that are being protected.
    pub rules: [FileRule; MAX_FILE_RULES],
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for Profile {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct File {
    /// Inode ID of the file.
    pub inode: u64,
    /// The device this file is located on.
    pub dev: u64,
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
    /// The operations that are permitted.
    ///
    /// Allowed operations have their corresponding bit set.
    pub permissions: u32,
    /// This rule is for a directory.
    pub is_dir: u32,
    /// The file's path.
    pub path: [u8; MAX_BUFFER_LEN],
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for FileRule {}

impl Default for FileRule {
    fn default() -> Self {
        Self {
            path: [0u8; MAX_BUFFER_LEN],
            is_dir: FileRule::IS_FILE,
            permissions: Self::NO_OPERATION,
        }
    }
}

impl FileRule {
    pub const IS_DIR: u32 = 1;
    pub const IS_FILE: u32 = 0;
    pub const NO_OPERATION: u32 = 0x00;
    pub const OPEN_MASK: u32 = 0x01 << 0;
    pub const READ_MASK: u32 = 0x01 << 1;
    pub const WRITE_MASK: u32 = 0x01 << 2;
    pub const EXEC_MASK: u32 = 0x01 << 3;
}

pub struct FileCacheKey {
    pub task: u64,
    pub target: u64,
}
