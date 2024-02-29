#![no_std]

#[repr(C)]
pub struct IpPortKey {
    pub ip: u32,
    pub port: u32,
}