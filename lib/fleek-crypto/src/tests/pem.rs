use crate::{AccountOwnerSecretKey, SecretKey};

#[test]
fn account_owner() {
    let key = AccountOwnerSecretKey::generate();
    let pem = key.encode_pem();
    let decoded = AccountOwnerSecretKey::decode_pem(&pem);
    assert_eq!(key, decoded);
}
