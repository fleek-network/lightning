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

/// Input encoding.
#[derive(Deserialize, Serialize)]
#[repr(u8)]
pub enum Encoding {
    /// Data is an 8-bit unsigned-int array.
    ///
    /// This input will be interpreted as an u8 array of dimension 1
    /// before feeding it to the model.
    #[serde(rename = "raw")]
    Raw,
    /// Data is a npy file.
    ///
    /// This input will be converted into an `ndarray`.
    #[serde(rename = "npy")]
    Npy,
    /// Data is encoded using Borsh.
    #[serde(rename = "borsh")]
    Borsh,
}

impl TryFrom<i8> for Encoding {
    type Error = std::io::Error;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        if value < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid encoding {value}",
            ));
        }
        let encoding = match value {
            0 => Encoding::Raw,
            1 => Encoding::Npy,
            2 => Encoding::Borsh,
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
