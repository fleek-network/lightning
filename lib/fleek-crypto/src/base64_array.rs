//! A custom serde serialize deserialize that uses base64 encoding when dealing with a
//! human readable serializer/deserialize. And uses a fixed size array when dealing
//! dealing with other things.
//!
//! # Example
//!
//! ```ignore
//! #[derive(Serializer, Deserialize)]
//! struct Data(#[serde(with = "base64_array") [u8; 128])
//! ```

use fastcrypto::encoding::{Base64, Encoding};
use serde::{Deserialize, Deserializer, Serializer};
use serde_big_array::BigArray;

pub fn serialize<const N: usize, S>(data: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        let base64 = Base64::encode(data);
        serializer.serialize_str(&base64)
    } else {
        // This uses `BigArray::serialize`.
        data.serialize(serializer)
    }
}

pub fn deserialize<'de, const N: usize, D>(deserializer: D) -> Result<[u8; N], D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let str: String = String::deserialize(deserializer)?;
        let vec = Base64::decode(&str).map_err(serde::de::Error::custom)?;

        if vec.len() != N {
            return Err(serde::de::Error::custom("Invalid size."));
        }

        let mut result = [0; N];
        result.copy_from_slice(&vec);
        Ok(result)
    } else {
        BigArray::deserialize(deserializer)
    }
}
