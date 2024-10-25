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
use libsecp256k1::{RecoveryId, Signature};
use serde::{Deserialize, Deserializer};
use serde_json::json;
use sgxkit::crypto::DerivedKey;
use sgxkit::io::output_writer;
use sgxkit::println;
use sha3::{Digest, Keccak256};

/// List of valid signing ethereum accounts
const PUBLIC_SIGNERS: &[&str] = &[
    "0xcd09987ab8141ec5b3646962e6810671d8f82434",
    "0xfc8c3f631b82f20e54ae5f3547054f63c7f97db5",
    "0x5a742918200f07183449b0ccba8b39818e269bac",
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

    let mut sigs = Vec::with_capacity(inputs.len());
    for v in inputs {
        println!("decoding hex");
        let raw: [u8; 65] = hex::decode(v.trim_start_matches("0x"))
            .map_err(|e| serde::de::Error::custom(e.to_string()))?
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid signature length"))?;

        println!("parsing signature");

        let signature = Signature::parse_standard(array_ref![raw, 0, 64])
            .map_err(|_| serde::de::Error::custom("invalid signature"))?;

        println!("parsing recovery id");

        let rid = RecoveryId::parse_rpc(raw[64])
            .map_err(|_| serde::de::Error::custom("invalid ethereum recovery id"))?;

        println!("done");

        sigs.push((signature, rid));
    }

    if sigs.len() < MINIMUM_SIGNATURES {
        return Err(serde::de::Error::custom("not enough signatures"));
    }

    Ok(sigs)
}

#[sgxkit::main]
pub fn main() -> Result<(), String> {
    println!("starting");
    // Get json data and parse into input
    let input = sgxkit::io::get_input_data_string().unwrap();

    println!("got input");

    let InputData {
        payload,
        signatures,
    } = serde_json::from_str(&input).map_err(|e| e.to_string())?;

    println!("decoded input");

    // Hash input
    let hash = Keccak256::digest(&payload);
    let msg = libsecp256k1::Message::parse(&hash.into());

    println!("hashed message: 0x{}", hex::encode(hash));

    let mut count = 0;
    for (sig, rid) in signatures {
        // Recover the signature's public key
        let pubkey = libsecp256k1::recover(&msg, &sig, &rid).map_err(|e| e.to_string())?;
        let hash = Keccak256::digest(pubkey.serialize());
        let eth = format!("0x{}", hex::encode(&hash[12..]));

        println!("recovered signer: {eth}");

        // Ensure public key is in the approved list
        if !PUBLIC_SIGNERS.contains(&eth.as_str()) {
            return Err(format!("Invalid signer {eth}"));
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
    let eth = format!("0x{}", hex::encode(&hash[12..]));

    println!("signing with key {eth}");

    // Sign the request data with the derived key
    let wasm_sig = key.sign_eth(&payload).map_err(|e| e.to_string())?;

    let res = json!({
        "account": eth,
        "signature": wasm_sig,
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
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn generate_example_request() {
        use base64::Engine;
        use sha3::Digest;

        // Create some secret keys
        let keys = (1u8..=3)
            .into_iter()
            .map(|v| libsecp256k1::SecretKey::parse(&[v; 32]).unwrap())
            .collect::<Vec<_>>();

        // Make a payload and hash it into a message to sign
        let payload = b"FOOBAR";
        let hash = sha3::Keccak256::digest(payload);
        let msg = libsecp256k1::Message::parse(&hash.into());

        // Sign the message with each key and encode as ethereum signatures
        let signatures = keys
            .iter()
            .map(|sk| {
                // print ethereum account ids for debugging
                let pk = libsecp256k1::PublicKey::from_secret_key(&sk);
                let hash = sha3::Keccak256::digest(pk.serialize());
                println!("0x{}", hex::encode(&hash[12..]));

                libsecp256k1::sign(&msg, &sk)
            })
            .map(|(sig, rid)| {
                // serialize signature as `Hex [r . s . v + 27]`
                let mut buf = sig.serialize().to_vec();
                buf.push(rid.serialize() + 27);
                format!("0x{}", hex::encode(&buf))
            })
            .collect::<Vec<_>>();

        // encode json and escape quotes (for terminal pasting)
        let out = serde_json::to_string(&serde_json::json!({
            "payload": base64::prelude::BASE64_STANDARD.encode(&payload),
            "signatures": signatures
        }))
        .unwrap()
        .replace('"', "\\\"");

        println!("\n{out}");
    }
}
