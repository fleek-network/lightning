use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, IsVariant)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda(usize),
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

impl From<&str> for Origin {
    fn from(s: &str) -> Self {
        match s {
            "blake3" => Self::Blake3,
            "ipfs" => Self::Ipfs,
            "http" => Self::Http,
            _ => Self::Unknown,
        }
    }
}
