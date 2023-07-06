use std::{borrow::Borrow, fmt::Display, str::FromStr};

use arrayref::array_ref;
use fastcrypto::{
    bls12381::min_sig::{
        BLS12381KeyPair, BLS12381PrivateKey, BLS12381PublicKey, BLS12381Signature,
    },
    ed25519::{Ed25519KeyPair, Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature},
    encoding::{Base64, Encoding},
    hash::{HashFunction, Keccak256},
    secp256k1::{
        recoverable::Secp256k1RecoverableSignature, Secp256k1KeyPair, Secp256k1PrivateKey,
        Secp256k1PublicKey,
    },
    traits::{
        KeyPair, RecoverableSignature, RecoverableSigner, Signer, ToFromBytes, VerifyRecoverable,
        VerifyingKey,
    },
};
use rand::rngs::ThreadRng;
use sec1::{
    pem,
    pkcs8::{der::EncodePem, AlgorithmIdentifierRef, ObjectIdentifier, PrivateKeyInfo},
    EcParameters, EcPrivateKey,
};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

#[cfg(test)]
mod tests;

pub trait PublicKey: Sized {
    type Signature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool;
    fn to_base64(&self) -> String;
    fn from_base64(encoded: &str) -> Option<Self>;
}

pub trait SecretKey: Sized {
    type PublicKey: PublicKey;

    fn generate() -> Self;
    fn decode_pem(encoded: &str) -> Option<Self>;
    fn encode_pem(&self) -> String;
    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature;
    fn to_pk(&self) -> Self::PublicKey;
}

const BLS12_381_PEM_LABEL: &str = "DRACO BLS12_381 PRIVATE KEY";

/// A node's BLS 12-381 public key
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodePublicKey(#[serde(with = "BigArray")] pub [u8; 96]);

impl From<[u8; 96]> for NodePublicKey {
    fn from(value: [u8; 96]) -> Self {
        Self(value)
    }
}

impl From<BLS12381PublicKey> for NodePublicKey {
    fn from(value: BLS12381PublicKey) -> Self {
        let bytes = value.as_ref();
        NodePublicKey(*array_ref!(bytes, 0, 96))
    }
}

impl From<&NodePublicKey> for BLS12381PublicKey {
    fn from(value: &NodePublicKey) -> Self {
        BLS12381PublicKey::from_bytes(&value.0).unwrap()
    }
}

impl PublicKey for NodePublicKey {
    type Signature = NodeSignature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool {
        let pubkey: BLS12381PublicKey = self.into();
        let signature: BLS12381Signature = signature.into();
        pubkey.verify(digest, &signature).is_ok()
    }

    fn to_base64(&self) -> String {
        Base64::encode(self.0)
    }

    fn from_base64(encoded: &str) -> Option<Self> {
        let bytes = Base64::decode(encoded).ok()?;
        (bytes.len() == 96).then(|| Self(*array_ref!(bytes, 0, 96)))
    }
}

impl Display for NodePublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// A node's BLS 12-381 secret key
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeSecretKey(#[serde(with = "BigArray")] pub [u8; 32]);

impl From<BLS12381PrivateKey> for NodeSecretKey {
    fn from(value: BLS12381PrivateKey) -> Self {
        let bytes = value.as_ref();
        NodeSecretKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&NodeSecretKey> for BLS12381PrivateKey {
    fn from(value: &NodeSecretKey) -> Self {
        BLS12381PrivateKey::from_bytes(&value.0).unwrap()
    }
}

impl From<NodeSecretKey> for BLS12381KeyPair {
    fn from(value: NodeSecretKey) -> Self {
        BLS12381PrivateKey::from(&value).into()
    }
}

impl SecretKey for NodeSecretKey {
    type PublicKey = NodePublicKey;

    fn generate() -> Self {
        let pair = BLS12381KeyPair::generate(&mut ThreadRng::default());
        pair.private().into()
    }

    /// Decode a BLS12-381 secret key from a custom protobuf pem file
    fn decode_pem(encoded: &str) -> Option<NodeSecretKey> {
        let (label, bytes) = sec1::pem::decode_vec(encoded.as_bytes()).ok()?;
        // todo: verify ec point
        (label == BLS12_381_PEM_LABEL && bytes.len() == 32)
            .then(|| NodeSecretKey(*array_ref!(bytes, 0, 32)))
    }

    /// Encode the BLS12-381 secret key to a custom protobuf pem file
    fn encode_pem(&self) -> String {
        sec1::pem::encode_string(BLS12_381_PEM_LABEL, sec1::LineEnding::LF, &self.0).unwrap()
    }

    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: BLS12381PrivateKey = self.into();
        secret.sign(digest).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: BLS12381PrivateKey = self.into();
        Into::<BLS12381PublicKey>::into(&secret).into()
    }
}

/// A node's BLS 12-381 signature
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeSignature(#[serde(with = "BigArray")] pub [u8; 48]);

impl From<BLS12381Signature> for NodeSignature {
    fn from(value: BLS12381Signature) -> Self {
        let bytes = value.as_ref();
        NodeSignature(*array_ref!(bytes, 0, 48))
    }
}

impl From<&NodeSignature> for BLS12381Signature {
    fn from(value: &NodeSignature) -> Self {
        BLS12381Signature::from_bytes(&value.0).unwrap()
    }
}

/// A node's ed25519 networking public key
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingPublicKey(pub [u8; 32]);

impl From<[u8; 32]> for NodeNetworkingPublicKey {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<Ed25519PublicKey> for NodeNetworkingPublicKey {
    fn from(value: Ed25519PublicKey) -> Self {
        let bytes = value.as_ref();
        NodeNetworkingPublicKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&NodeNetworkingPublicKey> for Ed25519PublicKey {
    fn from(value: &NodeNetworkingPublicKey) -> Self {
        Ed25519PublicKey::from_bytes(&value.0).unwrap()
    }
}

impl PublicKey for NodeNetworkingPublicKey {
    type Signature = NodeNetworkingSignature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool {
        let pubkey: Ed25519PublicKey = self.into();
        let signature: Ed25519Signature = signature.into();
        pubkey.verify(digest, &signature).is_ok()
    }

    fn to_base64(&self) -> String {
        Base64::encode(self.0)
    }

    fn from_base64(encoded: &str) -> Option<Self> {
        let bytes = Base64::decode(encoded).ok()?;
        (bytes.len() == 32).then(|| Self(*array_ref!(bytes, 0, 32)))
    }
}

impl Display for NodeNetworkingPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// A node's ed25519 networking secret key
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingSecretKey([u8; 32]);

impl From<Ed25519PrivateKey> for NodeNetworkingSecretKey {
    fn from(value: Ed25519PrivateKey) -> Self {
        let bytes = value.as_ref();
        NodeNetworkingSecretKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&NodeNetworkingSecretKey> for Ed25519PrivateKey {
    fn from(value: &NodeNetworkingSecretKey) -> Self {
        Ed25519PrivateKey::from_bytes(&value.0).unwrap()
    }
}

impl From<NodeNetworkingSecretKey> for Ed25519KeyPair {
    fn from(value: NodeNetworkingSecretKey) -> Self {
        let secret: Ed25519PrivateKey = (&value).into();
        secret.into()
    }
}

impl SecretKey for NodeNetworkingSecretKey {
    type PublicKey = NodeNetworkingPublicKey;

    fn generate() -> Self {
        let pair = Ed25519KeyPair::generate(&mut ThreadRng::default());
        pair.private().into()
    }

    fn decode_pem(encoded: &str) -> Option<Self> {
        let (label, der_bytes) = pem::decode_vec(encoded.as_bytes()).ok()?;
        if label != "PRIVATE KEY" {
            return None;
        }
        let info = PrivateKeyInfo::try_from(der_bytes.as_ref()).ok()?;
        Some(Self(*array_ref!(info.private_key, 2, 32)))
    }

    fn encode_pem(&self) -> String {
        let algorithm = AlgorithmIdentifierRef {
            oid: ObjectIdentifier::from_str("1.3.101.112").unwrap(),
            parameters: None,
        };
        PrivateKeyInfo::new(algorithm, &{
            let mut key = vec![0x04, 0x20];
            key.append(&mut self.0.into());
            key
        })
        .to_pem(sec1::pkcs8::LineEnding::LF)
        .unwrap()
    }

    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: Ed25519PrivateKey = self.into();
        let pair: Ed25519KeyPair = secret.into();
        pair.sign(digest).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: &Ed25519PrivateKey = &self.into();
        let pubkey: Ed25519PublicKey = secret.into();
        pubkey.into()
    }
}

/// A node's ed25519 networking signature
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingSignature([u8; 32]);

impl From<Ed25519Signature> for NodeNetworkingSignature {
    fn from(value: Ed25519Signature) -> Self {
        let bytes = value.as_ref();
        NodeNetworkingSignature(*array_ref!(bytes, 0, 32))
    }
}

impl From<&NodeNetworkingSignature> for Ed25519Signature {
    fn from(value: &NodeNetworkingSignature) -> Self {
        Ed25519Signature::from_bytes(&value.0).unwrap()
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientPublicKey(pub [u8; 20]);

impl PublicKey for ClientPublicKey {
    type Signature = ClientSignature;

    fn verify(&self, _signature: &Self::Signature, _digest: &[u8; 32]) -> bool {
        todo!()
    }

    fn to_base64(&self) -> String {
        Base64::encode(self.0)
    }

    fn from_base64(encoded: &str) -> Option<Self> {
        let bytes = Base64::decode(encoded).ok()?;
        (bytes.len() == 20).then(|| Self(*array_ref!(bytes, 0, 20)))
    }
}

impl Display for ClientPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientSecretKey([u8; 32]);

impl SecretKey for ClientSecretKey {
    type PublicKey = ClientPublicKey;

    fn generate() -> Self {
        todo!()
    }

    fn decode_pem(_encoded: &str) -> Option<ClientSecretKey> {
        todo!()
    }

    fn encode_pem(&self) -> String {
        todo!()
    }

    fn sign(&self, _digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        todo!()
    }

    fn to_pk(&self) -> Self::PublicKey {
        todo!()
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientSignature;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerPublicKey(#[serde(with = "BigArray")] pub [u8; 33]);

impl From<[u8; 33]> for AccountOwnerPublicKey {
    fn from(value: [u8; 33]) -> Self {
        Self(value)
    }
}

impl From<Secp256k1PublicKey> for AccountOwnerPublicKey {
    fn from(value: Secp256k1PublicKey) -> Self {
        let bytes = value.as_ref();
        AccountOwnerPublicKey(*array_ref!(bytes, 0, 33))
    }
}

impl From<&AccountOwnerPublicKey> for Secp256k1PublicKey {
    fn from(value: &AccountOwnerPublicKey) -> Self {
        Secp256k1PublicKey::from_bytes(&value.0).unwrap()
    }
}

impl PublicKey for AccountOwnerPublicKey {
    type Signature = AccountOwnerSignature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool {
        let pubkey: Secp256k1PublicKey = self.into();
        pubkey.verify_recoverable(digest, &signature.into()).is_ok()
    }

    fn to_base64(&self) -> String {
        Base64::encode(self.0)
    }

    fn from_base64(encoded: &str) -> Option<Self> {
        let bytes = Base64::decode(encoded).ok()?;
        (bytes.len() == 33).then(|| Self(*array_ref!(bytes, 0, 33)))
    }
}

impl Display for AccountOwnerPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let address: EthAddress = self.into();
        write!(f, "0x{}", hex::encode(address))
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct EthAddress(pub [u8; 20]);

impl AsRef<[u8]> for EthAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<A: Borrow<AccountOwnerPublicKey>> From<A> for EthAddress {
    fn from(value: A) -> Self {
        let pubkey: Secp256k1PublicKey = value.borrow().into();
        // get the uncompressed serialization (1 byte prefix + 32 byte X + 32 byte Y)
        let uncompressed = &pubkey.pubkey.serialize_uncompressed();
        // Compute a 32 byte keccak256 hash, ignoring the prefix
        let hash = Keccak256::digest(&uncompressed[1..65]).digest;
        // return the last 20 bytes of the hash
        EthAddress(*array_ref!(hash, 12, 20))
    }
}

impl Display for EthAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self))
    }
}

impl EthAddress {
    pub fn verify(&self, signature: &AccountOwnerSignature, digest: &[u8; 32]) -> bool {
        let signature: Secp256k1RecoverableSignature = signature.into();
        match signature.recover(digest) {
            Ok(public_key) => {
                if public_key.verify_recoverable(digest, &signature).is_err() {
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

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerSecretKey(#[serde(with = "BigArray")] pub [u8; 32]);

impl From<Secp256k1PrivateKey> for AccountOwnerSecretKey {
    fn from(value: Secp256k1PrivateKey) -> Self {
        let bytes = value.as_ref();
        AccountOwnerSecretKey(*array_ref!(bytes, 0, 32))
    }
}

impl From<&AccountOwnerSecretKey> for Secp256k1PrivateKey {
    fn from(value: &AccountOwnerSecretKey) -> Self {
        Secp256k1PrivateKey::from_bytes(&value.0).unwrap()
    }
}

impl SecretKey for AccountOwnerSecretKey {
    type PublicKey = AccountOwnerPublicKey;

    fn generate() -> Self {
        let pair = Secp256k1KeyPair::generate(&mut ThreadRng::default());
        pair.private().into()
    }

    fn decode_pem(encoded: &str) -> Option<Self> {
        let (label, der_bytes) = pem::decode_vec(encoded.as_bytes()).ok()?;
        if label != "EC PRIVATE KEY" {
            return None;
        }
        let info = EcPrivateKey::try_from(der_bytes.as_ref()).ok()?;
        Some(Self(*array_ref!(info.private_key, 0, 32)))
    }

    fn encode_pem(&self) -> String {
        let pubkey = self.to_pk();
        EcPrivateKey {
            private_key: &self.0,
            parameters: Some(EcParameters::NamedCurve(
                ObjectIdentifier::new("1.3.132.0.10").unwrap(),
            )),
            public_key: Some(&pubkey.0),
        }
        .to_pem(sec1::pkcs8::LineEnding::LF)
        .unwrap()
    }

    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: Secp256k1PrivateKey = self.into();
        let pair: Secp256k1KeyPair = secret.into();
        pair.sign_recoverable(digest).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: &Secp256k1PrivateKey = &self.into();
        let pubkey: Secp256k1PublicKey = secret.into();
        pubkey.into()
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerSignature(#[serde(with = "BigArray")] pub [u8; 65]);

impl From<Secp256k1RecoverableSignature> for AccountOwnerSignature {
    fn from(value: Secp256k1RecoverableSignature) -> Self {
        let bytes = value.as_ref();
        AccountOwnerSignature(*array_ref!(bytes, 0, 65))
    }
}

impl From<&AccountOwnerSignature> for Secp256k1RecoverableSignature {
    fn from(value: &AccountOwnerSignature) -> Self {
        Secp256k1RecoverableSignature::from_bytes(&value.0).unwrap()
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum TransactionSender {
    Node(NodePublicKey),
    AccountOwner(EthAddress),
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum TransactionSignature {
    Node(NodeSignature),
    AccountOwner(AccountOwnerSignature),
}

impl From<NodeSignature> for TransactionSignature {
    fn from(value: NodeSignature) -> Self {
        TransactionSignature::Node(value)
    }
}

impl From<AccountOwnerSignature> for TransactionSignature {
    fn from(value: AccountOwnerSignature) -> Self {
        TransactionSignature::AccountOwner(value)
    }
}

impl From<NodePublicKey> for TransactionSender {
    fn from(value: NodePublicKey) -> Self {
        TransactionSender::Node(value)
    }
}

impl<T: Into<EthAddress>> From<T> for TransactionSender {
    fn from(value: T) -> Self {
        TransactionSender::AccountOwner(value.into())
    }
}
