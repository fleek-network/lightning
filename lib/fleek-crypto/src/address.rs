use std::borrow::Borrow;
use std::fmt::Display;
use std::str::FromStr;

use arrayref::array_ref;
use derive_more::{AsRef, From};
use fastcrypto::hash::{HashFunction, Keccak256};
use fastcrypto::secp256k1::recoverable::Secp256k1RecoverableSignature;
use fastcrypto::secp256k1::Secp256k1PublicKey;
use fastcrypto::traits::{RecoverableSignature, VerifyRecoverable};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::hex_array;
use crate::keys::{AccountOwnerPublicKey, AccountOwnerSignature};

#[derive(
    From,
    AsRef,
    Debug,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Clone,
    Copy,
    Deserialize,
    JsonSchema,
    Default,
)]
pub struct EthAddress(#[serde(deserialize_with = "hex_array::deserialize")] pub [u8; 20]);

impl Serialize for EthAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        hex_array::serialize(&self.0, serializer)
    }
}

impl EthAddress {
    pub fn verify(&self, signature: &AccountOwnerSignature, digest: &[u8]) -> bool {
        let signature: Result<Secp256k1RecoverableSignature, _> = signature.try_into();
        let Ok(signature) = signature else {
            return false;
        };
        match signature.recover_with_hash::<Keccak256>(digest) {
            Ok(public_key) => {
                if public_key
                    .verify_recoverable_with_hash::<Keccak256>(digest, &signature)
                    .is_err()
                {
                    return false;
                }
                let public_key: AccountOwnerPublicKey = public_key.into();
                let eth_address: EthAddress = public_key.into();
                self.eq(&eth_address)
            },
            Err(_) => false,
        }
    }
}

impl<T> From<T> for EthAddress
where
    T: Borrow<AccountOwnerPublicKey>,
{
    fn from(value: T) -> Self {
        let pubkey: Secp256k1PublicKey = value.borrow().try_into().expect("invalid public key");
        // get the uncompressed serialization (1 byte prefix + 32 byte X + 32 byte Y)
        let uncompressed = &pubkey.pubkey.serialize_uncompressed();
        // Compute a 32 byte keccak256 hash, ignoring the prefix
        let hash = Keccak256::digest(&uncompressed[1..65]).digest;
        // return the last 20 bytes of the hash
        EthAddress(*array_ref!(hash, 12, 20))
    }
}

impl FromStr for EthAddress {
    type Err = std::io::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = match s.starts_with("0x") {
            true => &s[2..],
            false => s,
        };

        let bytes = hex::decode(s)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

        if bytes.len() != 20 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected 20 bytes",
            ));
        }

        let mut address = [0u8; 20];
        address.copy_from_slice(&bytes);
        Ok(EthAddress(address))
    }
}

impl Display for EthAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}
