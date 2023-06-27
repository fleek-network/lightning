use crate::{AccountOwnerSecretKey, SecretKey};

#[test]
fn account_owner_to_eth_address() {
    const TEST_KEY: &str = "-----BEGIN EC PRIVATE KEY-----
MFQCAQEEIIl2UnLXMkHBNj+J//IAytL/VwR6bxnI7ba99F22mHBloAcGBSuBBAAK
oSQDIgADbn0owIbD5+gHfyMLRqhrni5fryqoUsVdKwh2FZjWxLc=
-----END EC PRIVATE KEY-----";
    let key = AccountOwnerSecretKey::decode_pem(TEST_KEY).unwrap();
    let pubkey = key.to_pk();

    assert_eq!(
        pubkey.to_string(),
        "0x2d79dc4842e5c156a8b4387217d0c44ab3559548"
    );
}

mod pem {
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
        let decoded =
            NodeNetworkingSecretKey::decode_pem(&pem).expect("failed to decode ed25519 pem");
        assert_eq!(key, decoded);
    }

    #[test]
    fn account_owner_key_encode_decode() {
        let key = AccountOwnerSecretKey::generate();
        let pem = key.encode_pem();
        let decoded =
            AccountOwnerSecretKey::decode_pem(&pem).expect("failed to decode secp256k1 pem");
        assert_eq!(key, decoded);
    }
}
