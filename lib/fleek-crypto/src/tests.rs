use crate::{AccountOwnerSecretKey, EthAddress, SecretKey};

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

#[test]
fn test_verify_correct_eth_address() {
    let secret_key = AccountOwnerSecretKey::generate();
    let digest = [0; 32];
    let signature = secret_key.sign(&digest);
    let public_key = secret_key.to_pk();
    let eth_address: EthAddress = public_key.into();
    assert!(eth_address.verify(&signature, &digest));
}

#[test]
fn test_verify_false_eth_address() {
    let secret_key = AccountOwnerSecretKey::generate();
    let digest = [0; 32];
    let signature = secret_key.sign(&digest);

    // Create different eth address and make sure that the verification fails.
    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let eth_address: EthAddress = public_key.into();
    assert!(!eth_address.verify(&signature, &digest));
}

mod pem {
    use crate::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};

    #[test]
    fn node_key_encode_decode() {
        let key = ConsensusSecretKey::generate();
        let pem = key.encode_pem();
        let decoded = ConsensusSecretKey::decode_pem(&pem).expect("failed to decode bls12-381 pem");
        assert_eq!(key, decoded);
    }

    #[test]
    fn node_networking_key_encode_decode() {
        let key = NodeSecretKey::generate();
        let pem = key.encode_pem();
        let decoded = NodeSecretKey::decode_pem(&pem).expect("failed to decode ed25519 pem");
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
