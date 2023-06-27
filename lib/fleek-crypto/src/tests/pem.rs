use crate::{AccountOwnerSecretKey, NodeNetworkingSecretKey, NodeSecretKey, SecretKey};

#[test]
fn node_key_encode_decode() {
    let key = NodeSecretKey::generate();
    let pem = key.encode_pem();
    let decoded = NodeSecretKey::decode_pem(&pem).expect("failed to decode bls12-381 pem");
    assert_eq!(key, decoded);
}

#[test]
fn node_networking_key_encode_decode() {
    let key = NodeNetworkingSecretKey::generate();
    let pem = key.encode_pem();
    let decoded = NodeNetworkingSecretKey::decode_pem(&pem).expect("failed to decode ed25519 pem");
    assert_eq!(key, decoded);
}

#[test]
fn account_owner_key_encode_decode() {
    let key = AccountOwnerSecretKey::generate();
    let pem = key.encode_pem();
    let decoded = AccountOwnerSecretKey::decode_pem(&pem).expect("failed to decode secp256k1 pem");
    assert_eq!(key, decoded);
}
