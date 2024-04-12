#![no_std]

pub const MAX_DEVICES: usize = 4;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
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

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
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

#[derive(Copy, Clone)]
#[repr(C)]
pub struct File {
    pub inode_n: u64,
    pub dev: u32,
    pub rdev: u32,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for File {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FileMetadata {
    pub devs: [u32; MAX_DEVICES],
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for FileMetadata {}
