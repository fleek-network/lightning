// Todo: these should be imported from service but
// there is currently a linker-related issue
// that is preventing us.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda(usize),
}

#[derive(Deserialize, Serialize)]
pub struct StartSession {
    pub model: String,
    pub origin: Origin,
    pub device: Device,
}
