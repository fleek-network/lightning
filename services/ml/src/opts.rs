use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda,
}
