use crate::{AccountOwnerSecretKey, NodeNetworkingSecretKey, SecretKey};

#[test]
fn node_networking_encode_decode() {
    let key = NodeNetworkingSecretKey::generate();
    let pem = key.encode_pem();
    let decoded = NodeNetworkingSecretKey::decode_pem(&pem).expect("failed to decode pem");
    assert_eq!(key, decoded);
}

#[test]
fn account_owner_encode_decode() {
    let key = AccountOwnerSecretKey::generate();
    let pem = key.encode_pem();
    let decoded = AccountOwnerSecretKey::decode_pem(&pem).expect("failed to decode pem");
    assert_eq!(key, decoded);
}
