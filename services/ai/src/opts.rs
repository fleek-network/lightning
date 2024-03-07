use std::collections::HashMap;

use bytes::Bytes;
use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, IsVariant)]
#[repr(u8)]
pub enum Format {
    #[serde(rename = "bin")]
    Binary,
    #[serde(rename = "json")]
    Json,
}

#[derive(Debug, Deserialize, Serialize, IsVariant)]
pub enum Device {
    #[serde(rename = "cpu")]
    Cpu,
    // This is not supported atm.
    #[serde(rename = "cuda")]
    Cuda(usize),
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Origin {
    #[serde(rename = "blake3")]
    Blake3,
    #[serde(rename = "ipfs")]
    Ipfs,
    #[serde(rename = "http")]
    Http,
    #[serde(skip)]
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

#[derive(Deserialize, Serialize, Clone, Copy, IsVariant)]
#[repr(u8)]
pub enum Encoding {
    /// Data is a Borsh-encoded vector.
    #[serde(rename = "borsh")]
    Borsh = 0,
    /// Data is a safetensors-encoded ndarray.
    #[serde(rename = "safetensors")]
    SafeTensors = 1,
}

pub type BorshInput = HashMap<String, BorshVector>;

#[derive(Deserialize, Serialize)]
pub struct BorshVector {
    pub dtype: BorshVectorType,
    pub data: Bytes,
}

#[derive(Deserialize, Serialize, Clone, Copy)]
#[repr(u8)]
pub enum BorshVectorType {
    #[serde(rename = "i8")]
    Int8 = 0x00,
    #[serde(rename = "i16")]
    Int16 = 0x01,
    #[serde(rename = "i32")]
    Int32 = 0x02,
    #[serde(rename = "i64")]
    Int64 = 0x03,
    #[serde(rename = "u8")]
    Uint8 = 0x04,
    #[serde(rename = "u16")]
    Uint16 = 0x05,
    #[serde(rename = "u32")]
    Uint32 = 0x06,
    #[serde(rename = "u64")]
    Uint64 = 0x07,
    #[serde(rename = "32")]
    Float32 = 0x08,
    #[serde(rename = "f64")]
    Float64 = 0x09,
}

impl TryFrom<u8> for BorshVectorType {
    type Error = std::io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let encoding = match value {
            0x00 => BorshVectorType::Int8,
            0x01 => BorshVectorType::Int16,
            0x02 => BorshVectorType::Int32,
            0x03 => BorshVectorType::Int64,
            0x04 => BorshVectorType::Uint8,
            0x05 => BorshVectorType::Uint16,
            0x06 => BorshVectorType::Uint32,
            0x07 => BorshVectorType::Uint64,
            0x08 => BorshVectorType::Float32,
            0x09 => BorshVectorType::Float64,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unknown encoding {value}",
                ));
            },
        };

        Ok(encoding)
    }
}
