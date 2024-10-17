use std::io::Write;

use arrayref::array_ref;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use hex_literal::hex;
use libsecp256k1::{RecoveryId, Signature};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Deserializer};
use sgxkit::crypto::DerivedKey;
use sgxkit::io::output_writer;
use sgxkit::println;
use sha2::Digest;

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
    let mut hasher = sha2::Sha256::new();
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

    println!("signing with key");

    // Create a derived key and sign the request data with it
    let wasm_sig = DerivedKey::root()
        .sign(&payload)
        .map_err(|e| e.to_string())?;

    // Write base64 signature to certified output
    output_writer()
        .write_all(BASE64_STANDARD.encode(wasm_sig).as_bytes())
        .unwrap();

    Ok(())
}

#[cfg(all(test, target_family = "unix"))]
#[test]
fn generate_example_request() {
    let keys = (1u8..=3)
        .into_iter()
        .map(|v| libsecp256k1::SecretKey::parse(&[v; 32]).unwrap())
        .collect::<Vec<_>>();

    let payload = b"FOOBAR";
    let hash = sha2::Sha256::digest(payload);
    let msg = libsecp256k1::Message::parse(&hash.into());

    let signatures = keys
        .iter()
        .map(|sk| {
            let pk = libsecp256k1::PublicKey::from_secret_key(&sk);
            println!("{}", hex::encode(pk.serialize_compressed()));

            libsecp256k1::sign(&msg, &sk)
        })
        .map(|(sig, rid)| {
            let mut buf = sig.serialize().to_vec();
            buf.push(rid.serialize());
            BASE64_STANDARD.encode(&buf)
        })
        .collect::<Vec<_>>();

    let out = serde_json::to_string(&serde_json::json!({
        "payload": BASE64_STANDARD.encode(&payload),
        "signatures": signatures
    }))
    .unwrap()
    .replace('"', "\\\"");

    println!("\n{out}");
}
