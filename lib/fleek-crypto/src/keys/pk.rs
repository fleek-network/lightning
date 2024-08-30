use std::fmt::Display;
use std::str::FromStr;

use arrayref::array_ref;
use derive_more::{AsRef, From};
use fastcrypto::bls12381::min_sig::{
    BLS12381AggregateSignature,
    BLS12381PublicKey,
    BLS12381Signature,
};
use fastcrypto::ed25519::{Ed25519PublicKey, Ed25519Signature};
use fastcrypto::encoding::{Base58, Encoding};
use fastcrypto::secp256k1::recoverable::Secp256k1RecoverableSignature;
use fastcrypto::secp256k1::Secp256k1PublicKey;
use fastcrypto::traits::{ToFromBytes, VerifyRecoverable, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{base58_array, PublicKey};

macro_rules! impl_pk_sig {
    // If the name of the verify function is not provided. Default it to `verify`.
    (
        $pk_name:ident, $pk_size:expr, $pk_fc:ident,
        $sig_name:ident, $sig_size:expr, $sig_fc:ident
        $(, $aggr_sig_name:ident, $aggr_sig_size:expr, $aggr_sig_fc:ident)?
    ) => {
        impl_pk_sig!(
            $pk_name, $pk_size, $pk_fc, $sig_name, $sig_size, $sig_fc, verify
            $(, $aggr_sig_name, $aggr_sig_size, $aggr_sig_fc)?
        );
    };

    (
        $pk_name:ident, $pk_size:expr, $pk_fc:ident,
        $sig_name:ident, $sig_size:expr, $sig_fc:ident, $verify:ident
        $(, $aggr_sig_name:ident, $aggr_sig_size:expr, $aggr_sig_fc:ident)?
    ) => {
        #[derive(
            From, AsRef, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize,
        )]
        pub struct $pk_name(#[serde(with = "base58_array")] pub [u8; $pk_size]);

        #[derive(
            From, AsRef, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize,
        )]
        pub struct $sig_name(#[serde(with = "base58_array")] pub [u8; $sig_size]);

        $(
            #[derive(
                From, AsRef, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize,
                Deserialize,
            )]
            pub struct $aggr_sig_name(#[serde(with = "base58_array")] pub [u8; $aggr_sig_size]);
        )?

        impl PublicKey for $pk_name {
            type Signature = $sig_name;

            fn verify(&self, signature: &Self::Signature, digest: &[u8]) -> bool {
                let pubkey: $pk_fc = self.into();
                let signature: $sig_fc = signature.into();
                pubkey.$verify(digest, &signature.into()).is_ok()
            }

            fn to_base58(&self) -> String {
                Base58::encode(self.0)
            }

            fn from_base58(encoded: &str) -> Option<Self> {
                let bytes = Base58::decode(encoded).ok()?;
                (bytes.len() == $pk_size).then(|| Self(*arrayref::array_ref!(bytes, 0, $pk_size)))
            }
        }

        // <-- start of string, default, and display implementation.

        impl Display for $pk_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_base58())
            }
        }

        impl std::fmt::Debug for $pk_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    concat!(stringify!($pk_name), r#"("{}")"#),
                    self.to_base58()
                )
            }
        }

        impl FromStr for $pk_name {
            type Err = std::io::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let bytes = Base58::decode(s).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Public key not in base58 format.",
                    )
                })?;

                if bytes.len() != $pk_size {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid public key size.",
                    ));
                }

                Ok(Self(*array_ref!(bytes, 0, $pk_size)))
            }
        }

        impl Display for $sig_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", Base58::encode(self.0))
            }
        }

        impl std::fmt::Debug for $sig_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    concat!(stringify!($pk_name), r#"("{}")"#),
                    Base58::encode(self.0)
                )
            }
        }

        impl FromStr for $sig_name {
            type Err = std::io::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let bytes = Base58::decode(s).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Signature not in base58 format.",
                    )
                })?;

                if bytes.len() != $sig_size {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid signature size.",
                    ));
                }

                Ok(Self(*array_ref!(bytes, 0, $sig_size)))
            }
        }

        impl Default for $sig_name {
            fn default() -> Self {
                $sig_name([0; $sig_size])
            }
        }

        $(
            impl Display for $aggr_sig_name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", Base58::encode(self.0))
                }
            }

            impl std::fmt::Debug for $aggr_sig_name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(
                        f,
                        concat!(stringify!($pk_name), r#"("{}")"#),
                        Base58::encode(self.0)
                    )
                }
            }

            impl FromStr for $aggr_sig_name {
                type Err = std::io::Error;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    let bytes = Base58::decode(s).map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Aggregate signature not in base58 format.",
                        )
                    })?;

                    if bytes.len() != $aggr_sig_size {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Invalid aggregate signature size.",
                        ));
                    }

                    Ok(Self(*array_ref!(bytes, 0, $aggr_sig_size)))
                }
            }

            impl Default for $aggr_sig_name {
                fn default() -> Self {
                    $aggr_sig_name([0; $aggr_sig_size])
                }
            }
        )?

        // end of string and display implementation. -->

        // <-- start of fastcrypto conversions.

        impl From<$pk_fc> for $pk_name {
            fn from(value: $pk_fc) -> Self {
                let bytes = value.as_ref();
                $pk_name(*arrayref::array_ref!(bytes, 0, $pk_size))
            }
        }

        impl From<&$pk_name> for $pk_fc {
            fn from(value: &$pk_name) -> Self {
                $pk_fc::from_bytes(&value.0).unwrap()
            }
        }

        impl From<$sig_fc> for $sig_name {
            fn from(value: $sig_fc) -> Self {
                let bytes = value.as_ref();
                $sig_name(*array_ref!(bytes, 0, $sig_size))
            }
        }

        impl From<&$sig_name> for $sig_fc {
            fn from(value: &$sig_name) -> Self {
                $sig_fc::from_bytes(&value.0).unwrap()
            }
        }

        $(
            impl From<$aggr_sig_fc> for $aggr_sig_name {
                fn from(value: $aggr_sig_fc) -> Self {
                    let bytes = value.as_ref();
                    $aggr_sig_name(*array_ref!(bytes, 0, $aggr_sig_size))
                }
            }

            impl From<&$aggr_sig_name> for $aggr_sig_fc {
                fn from(value: &$aggr_sig_name) -> Self {
                    $aggr_sig_fc::from_bytes(&value.0).unwrap()
                }
            }
        )?

        // end of fastcrypto conversions -->

        // <-- start of aggregate signature implementation.

        $(
            use fastcrypto::traits::AggregateAuthenticator;

            impl std::borrow::Borrow<$sig_fc> for $sig_name {
                fn borrow(&self) -> &$sig_fc {
                    // SAFETY: This assumes that $sig_fc is $sig_name.
                    unsafe { &*(self as *const $sig_name as *const $sig_fc) }
                }
            }

            impl $aggr_sig_name {
                pub fn aggregate<'a, K, I>(signatures: I) -> anyhow::Result<Self>
                where
                    K: std::borrow::Borrow<$sig_name> + 'a,
                    I: IntoIterator<Item = &'a K>,
                {
                    let sig = $aggr_sig_fc::aggregate(
                        signatures.into_iter().map(|s| s.borrow().into()),
                    )?;
                    Ok(sig.into())
                }
            }
        )?

        // end of aggregate signature implementation. -->
    };
}

impl_pk_sig!(
    ConsensusPublicKey,
    96,
    BLS12381PublicKey,
    ConsensusSignature,
    48,
    BLS12381Signature,
    ConsensusAggregateSignature,
    48,
    BLS12381AggregateSignature
);

impl_pk_sig!(
    NodePublicKey,
    32,
    Ed25519PublicKey,
    NodeSignature,
    64,
    Ed25519Signature
);

impl_pk_sig!(
    AccountOwnerPublicKey,
    33,
    Secp256k1PublicKey,
    AccountOwnerSignature,
    65,
    Secp256k1RecoverableSignature,
    // Use a different method for signature verification than the default
    // `verify` method because we are using the recoverable signatures.
    verify_recoverable
);

impl_pk_sig!(
    ClientPublicKey,
    96,
    BLS12381PublicKey,
    ClientSignature,
    48,
    BLS12381Signature
);

impl schemars::JsonSchema for AccountOwnerSignature {
    fn schema_name() -> String {
        "AccountOwnerSignature".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::AccountOwnerSignature"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let sig = Self::from([0u8; 65]);

        schemars::schema_for_value!(sig).schema.into()
    }
}

impl schemars::JsonSchema for NodeSignature {
    fn schema_name() -> String {
        "NodeSignature".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::NodeSignature"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let sig = Self::from([0u8; 64]);

        schemars::schema_for_value!(sig).schema.into()
    }
}

impl schemars::JsonSchema for ConsensusSignature {
    fn schema_name() -> String {
        "ConsensusSignature".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::ConsensusSignature"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let sig = Self::from([0u8; 48]);

        schemars::schema_for_value!(sig).schema.into()
    }
}

impl schemars::JsonSchema for ClientPublicKey {
    fn schema_name() -> String {
        "ClientPublicKey".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::ClientPublicKey"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let key = ClientPublicKey::from_str("u76G7q22Qc5nRC5Fi6dzbNE7FQxqRKEtTS9qjDftWFwhBKmoozGLv8wFiFmGnYDFMEKyYxozWRdM3wgjs1Na3fvxDARxi9CSNJUZJfPXC2WUu3uLnUw96jPBRp7rtHEzS5H").expect("valid client public key for example");

        schemars::schema_for_value!(key).schema.into()
    }
}

impl schemars::JsonSchema for NodePublicKey {
    fn schema_name() -> String {
        "NodePublicKey".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::NodePublicKey"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let key = NodePublicKey::from_str("F5tV4PLSzx1Lt4mYBe13aYQ8hsLMTCfjgY2pLr82AumH")
            .expect("valid node public key for example");

        schemars::schema_for_value!(key).schema.into()
    }
}

impl schemars::JsonSchema for ConsensusPublicKey {
    fn schema_name() -> String {
        "ConsensusPublicKey".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::ConsensusPublicKey"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let key = ConsensusPublicKey::from_str("u76G7q22Qc5nRC5Fi6dzbNE7FQxqRKEtTS9qjDftWFwhBKmoozGLv8wFiFmGnYDFMEKyYxozWRdM3wgjs1Na3fvxDARxi9CSNJUZJfPXC2WUu3uLnUw96jPBRp7rtHEzS5H").expect("valid consensus public key for example");

        schemars::schema_for_value!(key).schema.into()
    }
}
