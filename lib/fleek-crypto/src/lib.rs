use arrayref::array_ref;
use fastcrypto::{
    bls12381::min_sig::{
        BLS12381KeyPair, BLS12381PrivateKey, BLS12381PublicKey, BLS12381Signature,
    },
    ed25519::{Ed25519KeyPair, Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature},
    secp256k1::{Secp256k1KeyPair, Secp256k1PrivateKey, Secp256k1PublicKey, Secp256k1Signature},
    traits::{KeyPair, Signer, ToFromBytes, VerifyingKey},
};
use rand::rngs::ThreadRng;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

pub trait PublicKey {
    type Signature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool;
}

pub trait SecretKey: Sized {
    type PublicKey: PublicKey;

    fn generate() -> Self;
    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature;
    fn to_pk(&self) -> Self::PublicKey;
}

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
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientSecretKey([u8; 32]);

impl SecretKey for ClientSecretKey {
    type PublicKey = ClientPublicKey;

    fn generate() -> Self {
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
pub struct AccountOwnerPublicKey(#[serde(with = "BigArray")] pub [u8; 32]);

impl From<[u8; 32]> for AccountOwnerPublicKey {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<Secp256k1PublicKey> for AccountOwnerPublicKey {
    fn from(value: Secp256k1PublicKey) -> Self {
        let bytes = value.as_ref();
        AccountOwnerPublicKey(*array_ref!(bytes, 1, 32))
    }
}

impl From<&AccountOwnerPublicKey> for Secp256k1PublicKey {
    fn from(value: &AccountOwnerPublicKey) -> Self {
        let mut bytes = Vec::with_capacity(33);
        bytes.push(0x04);
        bytes.append(&mut value.0.into());
        Secp256k1PublicKey::from_bytes(&bytes).unwrap()
    }
}

impl PublicKey for AccountOwnerPublicKey {
    type Signature = AccountOwnerSignature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool {
        let pubkey: Secp256k1PublicKey = self.into();
        pubkey.verify(digest, &signature.into()).is_ok()
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

    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        let secret: Secp256k1PrivateKey = self.into();
        let pair: Secp256k1KeyPair = secret.into();
        pair.sign(digest).into()
    }

    fn to_pk(&self) -> Self::PublicKey {
        let secret: &Secp256k1PrivateKey = &self.into();
        let pubkey: Secp256k1PublicKey = secret.into();
        pubkey.into()
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerSignature(#[serde(with = "BigArray")] pub [u8; 64]);

impl From<Secp256k1Signature> for AccountOwnerSignature {
    fn from(value: Secp256k1Signature) -> Self {
        let bytes = value.as_ref();
        AccountOwnerSignature(*array_ref!(bytes, 0, 64))
    }
}

impl From<&AccountOwnerSignature> for Secp256k1Signature {
    fn from(value: &AccountOwnerSignature) -> Self {
        Secp256k1Signature::from_bytes(&value.0).unwrap()
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum TransactionSender {
    Node(NodePublicKey),
    AccountOwner(AccountOwnerPublicKey),
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

impl From<AccountOwnerPublicKey> for TransactionSender {
    fn from(value: AccountOwnerPublicKey) -> Self {
        TransactionSender::AccountOwner(value)
    }
}
