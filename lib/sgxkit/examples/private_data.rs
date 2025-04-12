//! This example extracts private sealed data given by the user and returns it
//!
//! The data can either be encrypted for the shared key (given the wasm is approved),
//! or the wasm module's root key.
//!
//! # Example usage
//!
//! ```bash
//! # Build the example, with the PUBLIC_SIGNERS const set to the output of the above command
//! cargo build --target wasm32-unknown-unknown --example sgx-wasm-private-data -r
//!
//! # Put the content to node and get blake3 hash (can also use ipfs+fetcher to load, and b3sum to compute hash)
//! lightning-node dev store ./target/wasm32-unknown-unknown/release/examples/sgx-wasm-private-hash.wasm
//!
//! # Encrypt some data for the shared key with wasm approved, and encrypt with base64
//! echo "hi" | sgxencrypt shared --stdout - <hash> | base64 -w0
//!
//! # Or, encrypt some data for the wasm key and base64 encode it
//! echo "hi" | sgxencrypt wasm --stdout <hash> - | base64 -w0
//!
//! # Call the service with the data generated
//! curl localhost:4220/services/3 --data '{
//!   "input":"{\"type\":\"shared\",\"data\":\"<your base64 data>\"}",
//!   "hash":"<your hash>"
//! }'
//! ```

use std::io::Write;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Deserializer};
use sgxkit::crypto::{DerivedKey, SharedKey};
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

#[sgxkit::main]
fn main() -> anyhow::Result<()> {
    let Request {
        content_type,
        mut data,
    } = serde_json::from_str::<Request>(&get_input_data_string()?)?;
    match content_type {
        ContentType::Shared => SharedKey::unseal(&mut data)?,
        ContentType::Wasm => {
            DerivedKey::root().unseal(&mut data)?;
        },
    }
    output_writer().write_all(&data)?;
    Ok(())
}
