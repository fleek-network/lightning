#![no_std]

#[derive(Copy, Clone)]
#[repr(C)]
pub struct IpPortKey {
    pub ip: u32,
    pub port: u32,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for IpPortKey {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct File {
    pub inode_n: u64,
    pub dev: u32,
    pub rdev: u32,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for File {}
