use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, IsVariant)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda(usize),
}
