use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda(usize),
}
