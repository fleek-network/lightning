use std::fmt::Display;
use std::str::FromStr;

use arrayref::array_ref;
use derive_more::{AsRef, From};
use fastcrypto::bls12381::min_sig::{BLS12381PublicKey, BLS12381Signature};
use fastcrypto::ed25519::{Ed25519PublicKey, Ed25519Signature};
use fastcrypto::encoding::{Base64, Encoding};
use fastcrypto::secp256k1::recoverable::Secp256k1RecoverableSignature;
use fastcrypto::secp256k1::Secp256k1PublicKey;
use fastcrypto::traits::{ToFromBytes, VerifyRecoverable, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{base64_array, PublicKey};

macro_rules! impl_pk_sig {
    // If the name of the verify function is not provided. Default it to `verify`.
    (
        $pk_name:ident, $pk_size:expr, $pk_fc:ident,
        $sig_name:ident, $sig_size:expr, $sig_fc:ident
    ) => {
        impl_pk_sig!(
            $pk_name, $pk_size, $pk_fc, $sig_name, $sig_size, $sig_fc, verify
        );
    };

    (
        $pk_name:ident, $pk_size:expr, $pk_fc:ident,
        $sig_name:ident, $sig_size:expr, $sig_fc:ident,
        $verify:ident
    ) => {
        #[derive(
            From, AsRef, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize,
        )]
        pub struct $pk_name(#[serde(with = "base64_array")] pub [u8; $pk_size]);

        #[derive(
            From, AsRef, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize,
        )]
        pub struct $sig_name(#[serde(with = "base64_array")] pub [u8; $sig_size]);

        impl PublicKey for $pk_name {
            type Signature = $sig_name;

            fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool {
                let pubkey: $pk_fc = self.into();
                let signature: $sig_fc = signature.into();
                pubkey.$verify(digest, &signature.into()).is_ok()
            }

            fn to_base64(&self) -> String {
                Base64::encode(self.0)
            }

            fn from_base64(encoded: &str) -> Option<Self> {
                let bytes = Base64::decode(encoded).ok()?;
                (bytes.len() == $pk_size).then(|| Self(*arrayref::array_ref!(bytes, 0, $pk_size)))
            }
        }

        // <-- start of string and display implementation.

        impl Display for $pk_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_base64())
            }
        }

        impl std::fmt::Debug for $pk_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    concat!(stringify!($pk_name), r#"("{}")"#),
                    self.to_base64()
                )
            }
        }

        impl FromStr for $pk_name {
            type Err = std::io::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let bytes = Base64::decode(s).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Public key not in base64 format.",
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
                write!(f, "{}", Base64::encode(self.0))
            }
        }

        impl std::fmt::Debug for $sig_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    concat!(stringify!($pk_name), r#"("{}")"#),
                    Base64::encode(self.0)
                )
            }
        }

        impl FromStr for $sig_name {
            type Err = std::io::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let bytes = Base64::decode(s).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Signature not in base64 format.",
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

        // end of fastcrypto conversions -->
    };
}

impl_pk_sig!(
    ConsensusPublicKey,
    96,
    BLS12381PublicKey,
    ConsensusSignature,
    48,
    BLS12381Signature
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
