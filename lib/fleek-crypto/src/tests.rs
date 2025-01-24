use crate::{
    AccountOwnerSecretKey,
    ConsensusAggregateSignature,
    ConsensusPublicKey,
    ConsensusSecretKey,
    ConsensusSignature,
    EthAddress,
    FleekCryptoError,
    NodePublicKey,
    NodeSignature,
    PublicKey,
    SecretKey,
};

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
        "228FehnRsJbWjdP5VCW8teLLhPvhr1oBf6FngsszA44mk"
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

#[test]
fn test_consensus_aggregate_signature_verify() {
    let sk1 = ConsensusSecretKey::generate();
    let sk2 = ConsensusSecretKey::generate();
    let sk3 = ConsensusSecretKey::generate();

    let sig1 = sk1.sign(&[1; 32]);
    let sig2 = sk2.sign(&[2; 32]);
    let sig3 = sk3.sign(&[3; 32]);

    let agg_sig = ConsensusAggregateSignature::aggregate(&[&sig1, &sig2, &sig3]).unwrap();
    assert!(agg_sig
        .verify(
            &[sk1.to_pk(), sk2.to_pk(), sk3.to_pk()],
            &[&[1; 32], &[2; 32], &[3; 32]],
        )
        .unwrap());

    // Should return error if signature has invalid bytes.
    assert_eq!(
        ConsensusAggregateSignature::default()
            .verify(
                &[sk1.to_pk(), sk2.to_pk(), sk3.to_pk()],
                &[&[1; 32], &[2; 32], &[3; 32]],
            )
            .unwrap_err(),
        FleekCryptoError::InvalidAggregateSignature(
            "Invalid value was given to the function".to_string()
        )
    );
}

#[test]
fn test_signature_verify_should_return_error() {
    let sk = NodePublicKey([1; 32]);
    let sig = NodeSignature([1; 64]);
    assert_eq!(
        sk.verify(&sig, &[0; 32]),
        Err(FleekCryptoError::InvalidSignature(
            "Invalid signature was given to the function".to_string()
        ))
    );

    let sk = ConsensusPublicKey([1; 96]);
    let sig = ConsensusSignature([1; 48]);
    assert_eq!(
        sk.verify(&sig, &[0; 32]),
        Err(FleekCryptoError::InvalidPublicKey(
            "Invalid value was given to the function".to_string()
        ))
    );
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

mod from_display {
    use std::any::type_name;
    use std::fmt::{Debug, Display};
    use std::str::FromStr;

    use crate::{
        AccountOwnerSecretKey,
        ConsensusAggregateSignature,
        ConsensusSecretKey,
        EthAddress,
        NodeSecretKey,
        PublicKey,
        SecretKey,
    };

    fn from_display_should_work<T>(data: T)
    where
        T: Display + FromStr + Eq,
        T::Err: Debug,
    {
        let string = format!("{}", data);
        let decoded = T::from_str(&string)
            .unwrap_or_else(|e| panic!("{}::from_str failed. err: {e:?}", type_name::<T>()));
        if decoded != data {
            panic!(
                "{}::FromDisplay failed.\n expected='{data}' actual='{decoded}'",
                type_name::<T>()
            );
        }
    }

    fn run_test<T: SecretKey>()
    where
        T::PublicKey: FromStr + Display + Eq,
        <T::PublicKey as PublicKey>::Signature: FromStr + Display + Eq,
        <<T::PublicKey as PublicKey>::Signature as FromStr>::Err: Debug,
        <T::PublicKey as FromStr>::Err: Debug,
    {
        let sk = T::generate();
        let pk = sk.to_pk();
        from_display_should_work(pk);
        let sig = sk.sign(&[0; 32]);
        from_display_should_work(sig);
    }

    #[test]
    fn account_owner() {
        run_test::<AccountOwnerSecretKey>();
        let addr: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();
        from_display_should_work(addr);
    }

    #[test]
    fn node() {
        run_test::<NodeSecretKey>();
    }

    #[test]
    fn consensus() {
        run_test::<ConsensusSecretKey>();

        // Test aggregate signature.
        let sk1 = ConsensusSecretKey::generate();
        let sk2 = ConsensusSecretKey::generate();
        let sk3 = ConsensusSecretKey::generate();

        let sig1 = sk1.sign(&[0; 32]);
        let sig2 = sk2.sign(&[0; 32]);
        let sig3 = sk3.sign(&[0; 32]);

        let agg_sig = ConsensusAggregateSignature::aggregate(&[&sig1, &sig2, &sig3]).unwrap();
        from_display_should_work(agg_sig);
    }
}

mod test_serde {
    use std::any::type_name;
    use std::fmt::Display;

    use serde::de::DeserializeOwned;
    use serde::Serialize;

    use crate::{
        AccountOwnerSecretKey,
        ConsensusAggregateSignature,
        ConsensusSecretKey,
        EthAddress,
        NodeSecretKey,
        PublicKey,
        SecretKey,
    };

    fn json_should_work<T>(data: &T)
    where
        T: Display + Eq + Serialize + DeserializeOwned,
    {
        let encoded = serde_json::to_string(data).expect("Failed to serialize with json.");

        // We use a human readable format for json/toml so we should not have serialized
        // the data as an array of numbers. So no `[...]` should exists in the serialization.
        assert!(
            !encoded.contains('['),
            "Json serailization must not contain an array: {}",
            encoded
        );

        let decoded = serde_json::from_str::<T>(&encoded).expect("Failed to deserialize with json");

        if decoded != *data {
            panic!(
                "{} failed. expected='{data}' actual='{decoded}'",
                type_name::<T>()
            );
        }
    }

    fn bincode_should_work<T>(data: &T)
    where
        T: Display + Eq + Serialize + DeserializeOwned,
    {
        let encoded = bincode::serialize(data).expect("Failed to serialize with bincode.");

        // smaller-or-equal because of memory alignment of a raw `T`.
        assert!(
            encoded.len() <= std::mem::size_of::<T>(),
            "Bincode serialization must use binary format."
        );

        let decoded =
            bincode::deserialize::<T>(&encoded).expect("Failed to deserialize with bincode");

        if decoded != *data {
            panic!(
                "{} failed. expected='{data}' actual='{decoded}'",
                type_name::<T>()
            );
        }
    }

    fn serde_should_work<T>(data: T)
    where
        T: Display + Eq + Serialize + DeserializeOwned,
    {
        json_should_work(&data);
        bincode_should_work(&data);
    }

    fn run_test<T: SecretKey>()
    where
        T::PublicKey: Serialize + DeserializeOwned + Display + Eq,
        <T::PublicKey as PublicKey>::Signature: Serialize + DeserializeOwned + Display + Eq,
    {
        let sk = T::generate();
        let pk = sk.to_pk();
        serde_should_work(pk);
        let sig = sk.sign(&[0; 32]);
        serde_should_work(sig);
    }

    #[test]
    fn account_owner() {
        run_test::<AccountOwnerSecretKey>();
        let addr: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();
        serde_should_work(addr);
    }

    #[test]
    fn node() {
        run_test::<NodeSecretKey>();
    }

    #[test]
    fn consensus() {
        run_test::<ConsensusSecretKey>();

        // Test aggregate signature.
        let sk1 = ConsensusSecretKey::generate();
        let sk2 = ConsensusSecretKey::generate();
        let sk3 = ConsensusSecretKey::generate();

        let sig1 = sk1.sign(&[0; 32]);
        let sig2 = sk2.sign(&[0; 32]);
        let sig3 = sk3.sign(&[0; 32]);

        let agg_sig = ConsensusAggregateSignature::aggregate(&[&sig1, &sig2, &sig3]).unwrap();
        serde_should_work(agg_sig);
    }
}
