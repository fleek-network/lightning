use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, IsVariant)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda(usize),
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
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

#[derive(Deserialize, Serialize, Clone, Copy)]
#[repr(u8)]
pub enum Encoding {
    BorshInt8 = 0x00,
    BorshInt16 = 0x01,
    BorshInt32 = 0x02,
    BorshInt64 = 0x03,
    BorshUint8 = 0x04,
    BorshUint16 = 0x05,
    BorshUint32 = 0x06,
    BorshUint64 = 0x07,
    BorshFloat32 = 0x08,
    BorshFloat64 = 0x09,
    SafeTensors = 0x0A,
}

impl TryFrom<u8> for Encoding {
    type Error = std::io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let encoding = match value {
            0x00 => Encoding::BorshInt8,
            0x01 => Encoding::BorshInt16,
            0x02 => Encoding::BorshInt32,
            0x03 => Encoding::BorshInt64,
            0x04 => Encoding::BorshUint8,
            0x05 => Encoding::BorshUint16,
            0x06 => Encoding::BorshUint32,
            0x07 => Encoding::BorshUint64,
            0x08 => Encoding::BorshFloat32,
            0x09 => Encoding::BorshFloat64,
            0x0A => Encoding::SafeTensors,
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
