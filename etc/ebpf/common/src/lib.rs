#![no_std]

#[derive(Copy, Clone)]
#[repr(C)]
pub struct IpPortKey {
    pub ip: u32,
    pub port: u32,
}

#[cfg(feature = "userspace")]
unsafe impl aya::Pod for IpPortKey {}
