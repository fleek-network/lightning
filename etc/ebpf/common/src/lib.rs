#![no_std]

pub const MAX_DEVICES: usize = 4;

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

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FileMetadata {
    pub devs: [u32; MAX_DEVICES],
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for FileList {}
