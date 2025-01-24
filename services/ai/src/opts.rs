use std::collections::HashMap;
use std::str::FromStr;

use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

pub enum Service {
    Inference,
    Info,
}

impl FromStr for Service {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let service = match s {
            "infer" => Self::Inference,
            "info" => Self::Info,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unknown service",
                ));
            },
        };
        Ok(service)
    }
}

#[derive(Debug, Deserialize, Serialize, IsVariant)]
#[repr(u8)]
pub enum Format {
    #[serde(rename = "bin")]
    Binary,
    #[serde(rename = "json")]
    Json,
}

impl FromStr for Format {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let format = match s {
            "bin" => Self::Binary,
            "json" => Self::Json,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unknown format",
                ));
            },
        };
        Ok(format)
    }
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

impl FromStr for Encoding {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let format = match s {
            "borsh" => Self::Borsh,
            "safetensors" => Self::SafeTensors,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unknown encoding",
                ));
            },
        };
        Ok(format)
    }
}

pub type BorshNamedVectors = HashMap<String, BorshVector>;

#[derive(Deserialize, Serialize)]
pub struct BorshVector {
    pub dtype: BorshVectorType,
    #[serde(with = "borsh_base64")]
    pub data: Vec<u8>,
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
    #[serde(rename = "f32")]
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
                    format!("unknown encoding {value}"),
                ));
            },
        };

        Ok(encoding)
    }
}

mod borsh_base64 {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&base64::display::Base64Display::new(
            v,
            &base64::prelude::BASE64_STANDARD,
        ))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let base64 = String::deserialize(d)?;
        base64::prelude::BASE64_STANDARD
            .decode(base64.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}
