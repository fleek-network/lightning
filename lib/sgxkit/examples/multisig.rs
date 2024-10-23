//! Multisig Example
//!
//! # Example usage
//!
//! ```bash
//! # Run the test to generate some public keys and signatures
//! cargo test --example sgx-wasm-multisig --target x86_64-unknown-linux-gnu -- --nocapture
//!
//! # Build the example, with the PUBLIC_SIGNERS const set to the output of the above command
//! cargo build --target wasm32-unknown-unknown --example sgx-wasm-multisig -r
//!
//! # Put the content to node and get blake3 hash (can also use ipfs+fetcher to load, and b3sum to compute hash)
//! lightning-node dev store ./target/wasm32-unknown-unknown/release/examples/sgx-wasm-multisig.wasm
//!
//! # Call the service with the example signatures generated above
//! curl localhost:4220/services/3 --data '{
//!   "input":"{\"payload\":\"Rk9PQkFS\",\"signatures\":[\"C/FAWjjggEOZAjqWyxcF0PwiOIfcK1grt5zCk9rj2o8MEhl0Lxodu1h7Id3zSkByu3eZpqYgbNNB/hE1qem9DAE=\",\"7Eb6Z2knL4vhHjJUkfBEdHzIRvk0vTSq/K+h6M3JZwNHiIRCwh1V5Y+idXlGsWVy1c8+ha3A5Z1pcU0YyYRZdgE=\",\"0GXICyZa6cMHJthU1e2sSH76F5o7aLzJz8H9gU7UqX9nV5AslcXd8YwG79eXbzU2TrBPJWNvEV1ee46Rm/ybPAA=\"]}",
//!   "hash":"<your hash>"
//! }'
//! ```

use std::io::Write;

use arrayref::array_ref;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use hex_literal::hex;
use libsecp256k1::{RecoveryId, Signature};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use sgxkit::crypto::DerivedKey;
use sgxkit::io::output_writer;
use sgxkit::println;
use sha2::{Digest, Sha256};
use sha3::Keccak256;

/// List of valid secp256k1 signers
const PUBLIC_SIGNERS: &[[u8; 33]] = &[
    hex!("031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"),
    hex!("024d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766"),
    hex!("02531fe6068134503d2723133227c867ac8fa6c83c537e9a44c3c5bdbdcb1fe337"),
];

/// Require ceil(signers * 2 / 3) valid signatures
const MINIMUM_SIGNATURES: usize = (PUBLIC_SIGNERS.len() * 2).div_ceil(3);

#[derive(Deserialize)]
struct InputData {
    /// Base64 encoded payload
    #[serde(deserialize_with = "decode_base64")]
    payload: Vec<u8>,
    /// Array of base64 encoded 65 byte recoverable signatures
    #[serde(deserialize_with = "decode_signatures")]
    signatures: Vec<(Signature, RecoveryId)>,
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

/// Decode `Vec<Base64<[r . s . v]>>`
fn decode_signatures<'de, D>(de: D) -> Result<Vec<(Signature, RecoveryId)>, D::Error>
where
    D: Deserializer<'de>,
{
    let inputs = Vec::<String>::deserialize(de)?;
    if inputs.len() < MINIMUM_SIGNATURES {
        return Err(serde::de::Error::custom(format!(
            "must have {MINIMUM_SIGNATURES} signatures",
        )));
    }

    let mut sigs = vec![];
    for base in inputs {
        let raw: [u8; 65] = decode_base64(base.into_deserializer())?
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid signature length"))?;
        let signature = Signature::parse_standard(array_ref![raw, 0, 64])
            .map_err(|_| serde::de::Error::custom("invalid signature"))?;
        let rid = RecoveryId::parse(raw[64])
            .map_err(|_| serde::de::Error::custom("invalid recovery id"))?;
        sigs.push((signature, rid));
    }

    Ok(sigs)
}

#[sgxkit::main]
pub fn main() -> Result<(), String> {
    // Get json data and parse into input
    let input = sgxkit::io::get_input_data_string().unwrap();
    let InputData {
        payload,
        signatures,
    } = serde_json::from_str(&input).map_err(|e| e.to_string())?;

    println!("decoded input");

    // Hash input
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    let hash = hasher.finalize();
    let msg = libsecp256k1::Message::parse(&hash.into());

    println!("hashed message");

    let mut count = 0;
    for (sig, rid) in signatures {
        // Recover the signature's public key
        let pubkey = libsecp256k1::recover(&msg, &sig, &rid).map_err(|e| e.to_string())?;

        println!(
            "recovered pubkey: {}",
            hex::encode(pubkey.serialize_compressed())
        );

        // Ensure public key is in the approved list
        if !PUBLIC_SIGNERS.contains(&pubkey.serialize_compressed()) {
            return Err(format!(
                "Invalid signer {}",
                hex::encode(pubkey.serialize_compressed())
            ));
        }

        // Verify the signature
        if !libsecp256k1::verify(&msg, &sig, &pubkey) {
            return Err("Invalid signature".to_string());
        }

        println!("verified signature");
        count += 1;
    }

    if count < MINIMUM_SIGNATURES {
        return Err("Not enough valid signatures".to_string());
    }

    let key = DerivedKey::root();

    // Encode public key as ethereum account id
    // - hash uncompressed with keccack256
    // - take last 20 bytes (drop the first 12)
    // - hex encode with 0x prefix
    let compressed = key.public_key().unwrap();
    let uncompressed = libsecp256k1::PublicKey::parse_compressed(&compressed)
        .unwrap()
        .serialize();
    let hash = Keccak256::digest(uncompressed);
    let pk = format!("0x{}", hex::encode_upper(&hash[12..]));

    println!("signing with key {pk}");

    // Sign the request data with the derived key
    let wasm_sig = key.sign(&payload).map_err(|e| e.to_string())?;

    let res = json!({
        "account": pk,
        "signature": BASE64_STANDARD.encode(wasm_sig),
    });

    // Write to certified output
    output_writer()
        .write_all(serde_json::to_string_pretty(&res).unwrap().as_bytes())
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {
    /// Generate some example data on a host system
    #[cfg(target_family = "unix")]
    #[test]
    fn generate_example_request() {
        // Create some secret keys
        let keys = (1u8..=3)
            .into_iter()
            .map(|v| libsecp256k1::SecretKey::parse(&[v; 32]).unwrap())
            .collect::<Vec<_>>();

        // Make a payload and hash it into a message to sign
        let payload = b"FOOBAR";
        let hash = Sha256::digest(payload);
        let msg = libsecp256k1::Message::parse(&hash.into());

        // Sign the message with each key
        let signatures = keys
            .iter()
            .map(|sk| {
                // print pubkeys for debugging
                let pk = libsecp256k1::PublicKey::from_secret_key(&sk);
                println!("{}", hex::encode(pk.serialize_compressed()));

                libsecp256k1::sign(&msg, &sk)
            })
            .map(|(sig, rid)| {
                // serialize signature as `Base64<[r . s . v]>`
                let mut buf = sig.serialize().to_vec();
                buf.push(rid.serialize());
                BASE64_STANDARD.encode(&buf)
            })
            .collect::<Vec<_>>();

        // encode json and escape quotes (for terminal pasting)
        let out = serde_json::to_string(&serde_json::json!({
            "payload": BASE64_STANDARD.encode(&payload),
            "signatures": signatures
        }))
        .unwrap()
        .replace('"', "\\\"");

        println!("\n{out}");
    }
}
