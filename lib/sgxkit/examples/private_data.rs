//! This example computes the hash of some private sealed data given by the user.
//!
//! The data can either be encrypted for the shared key (given the wasm is approved),
//! or the root wasm key.

use std::io::Write;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Deserializer};
use sgxkit::crypto::{shared_key_unseal, DerivedKey};
use sgxkit::io::{get_input_data_string, output_writer};

#[derive(Deserialize, strum::EnumString)]
#[strum(ascii_case_insensitive)]
#[serde(try_from = "&str")]
enum ContentType {
    Shared,
    Wasm,
}

#[derive(Deserialize)]
struct Request {
    #[serde(rename = "type")]
    content_type: ContentType,
    #[serde(deserialize_with = "decode_base64")]
    data: Vec<u8>,
}

fn decode_base64<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let base = String::deserialize(de)?;
    BASE64_STANDARD
        .decode(base)
        .map_err(|e| serde::de::Error::custom(e.to_string()))
}

fn main() -> anyhow::Result<()> {
    let Request {
        content_type,
        mut data,
    } = serde_json::from_str::<Request>(&get_input_data_string()?)?;
    match content_type {
        ContentType::Shared => shared_key_unseal(&mut data)?,
        ContentType::Wasm => {
            DerivedKey::root().unseal(&mut data)?;
        },
    }
    let hash = blake3::hash(&data).to_string();
    output_writer().write_all(hash.as_bytes())?;
    Ok(())
}
