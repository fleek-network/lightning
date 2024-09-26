use std::io::Write;

use arrayref::array_ref;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use hex_literal::hex;
use k256::ecdsa::signature::Verifier;
use k256::ecdsa::{RecoveryId, Signature};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Deserializer};
use sgxkit::crypto::DerivedKey;
use sgxkit::io::OutputWriter;

/// List of valid secp256k1 signers
const PUBLIC_SIGNERS: &[[u8; 33]] = &[
    hex!("031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"),
    hex!("024d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766"),
    hex!("02531fe6068134503d2723133227c867ac8fa6c83c537e9a44c3c5bdbdcb1fe337"),
];

/// Require ceil(signers * 2 / 3) valid signatures
const MINIMUM_SIGNATURES: usize = const { (PUBLIC_SIGNERS.len() * 2).div_ceil(3) };

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

/// Decode `Seq<Base64( [r . s . v] )>`
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
        let signature = Signature::from_bytes(array_ref![raw, 0, 64].into())
            .map_err(|_| serde::de::Error::custom("invalid signature"))?;
        let rid = RecoveryId::from_byte(raw[64])
            .ok_or(serde::de::Error::custom("invalid recovery id"))?;
        sigs.push((signature, rid));
    }

    Ok(sigs)
}

pub fn main() -> Result<(), String> {
    // Get json data and parse into input
    let input = sgxkit::io::get_input_data_string().unwrap();
    let InputData {
        payload,
        signatures,
    } = serde_json::from_str(&input).map_err(|e| e.to_string())?;

    let count = 0;
    for (sig, rid) in signatures {
        // Recover the signature's public key
        let pubkey = k256::ecdsa::VerifyingKey::recover_from_msg(&payload, &sig, rid)
            .map_err(|e| e.to_string())?;
        // Ensure public key is in the approved list
        if !PUBLIC_SIGNERS.contains(&pubkey.to_sec1_bytes().as_ref().try_into().unwrap()) {
            return Err(format!(
                "Invalid signer {}",
                hex::encode(pubkey.to_sec1_bytes().as_ref())
            ));
        }
        // Verify the signature
        pubkey.verify(&payload, &sig).map_err(|e| e.to_string())?;
    }

    if count < MINIMUM_SIGNATURES {
        return Err("Not enough valid signatures".to_string());
    }

    // Create a derived key and sign the request data with it
    let wasm_sig = DerivedKey::root()
        .sign(&payload)
        .map_err(|e| e.to_string())?;

    // Write base64 signature to certified output
    OutputWriter::new()
        .write_all(BASE64_STANDARD.encode(wasm_sig).as_bytes())
        .unwrap();

    Ok(())
}

#[cfg(all(test, target_family = "unix"))]
#[test]
fn generate_example_request() {
    use serde_json::json;

    let alice = k256::ecdsa::SigningKey::from_bytes(&[1u8; 32].into()).unwrap();
    let bob = k256::ecdsa::SigningKey::from_bytes(&[2u8; 32].into()).unwrap();
    let charlie = k256::ecdsa::SigningKey::from_bytes(&[3u8; 32].into()).unwrap();

    let keys = [alice, bob, charlie];

    println!(
        "{}",
        keys.iter()
            .map(|k| k.verifying_key().to_sec1_bytes())
            .map(hex::encode)
            .collect::<Vec<_>>()
            .join("\n")
    );

    let payload = b"FOOBAR";
    let signatures = keys
        .map(|k| k.sign_recoverable(payload).unwrap())
        .map(|(sig, rid)| {
            let mut buf = sig.to_bytes().to_vec();
            buf.push(rid.to_byte());
            BASE64_STANDARD.encode(&buf)
        });

    std::fs::write(
        "./examples/multisig.json",
        serde_json::to_string_pretty(&json!({
            "payload": BASE64_STANDARD.encode(&payload),
            "signatures": signatures
        }))
        .unwrap(),
    )
    .unwrap()
}
