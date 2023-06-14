use fastcrypto::{bls12381::min_sig::BLS12381PrivateKey, ed25519::Ed25519PrivateKey};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodePublicKey(#[serde(with = "BigArray")] pub [u8; 96]);
pub type NodeSecretKey = BLS12381PrivateKey;
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeSignature(#[serde(with = "BigArray")] pub [u8; 48]);

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingPublicKey(pub [u8; 32]);
pub type NodeNetworkingSecretKey = Ed25519PrivateKey;
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingSignature;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientPublicKey(pub [u8; 20]);
pub struct ClientSecretKey([u8; 32]);
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientSignature;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerPublicKey(pub [u8; 32]);
pub struct AccountOwnerSecretKey;
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerSignature;

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

pub trait SecretKey {
    type PublicKey: PublicKey;

    fn sign(&self, digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature;

    fn to_pk(&self) -> Self::PublicKey;
}

pub trait PublicKey {
    type Signature;

    fn verify(&self, signature: &Self::Signature, digest: &[u8; 32]) -> bool;
}

impl PublicKey for NodePublicKey {
    type Signature = NodeSignature;

    fn verify(&self, _signature: &Self::Signature, _digest: &[u8; 32]) -> bool {
        todo!()
    }
}

impl PublicKey for NodeNetworkingPublicKey {
    type Signature = NodeNetworkingSignature;

    fn verify(&self, _signature: &Self::Signature, _digest: &[u8; 32]) -> bool {
        todo!()
    }
}

impl PublicKey for ClientPublicKey {
    type Signature = ClientSignature;

    fn verify(&self, _signature: &Self::Signature, _digest: &[u8; 32]) -> bool {
        todo!()
    }
}

impl PublicKey for AccountOwnerPublicKey {
    type Signature = AccountOwnerSignature;

    fn verify(&self, _signature: &Self::Signature, _digest: &[u8; 32]) -> bool {
        todo!()
    }
}

impl SecretKey for NodeSecretKey {
    type PublicKey = NodePublicKey;

    fn sign(&self, _digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        todo!()
    }

    fn to_pk(&self) -> Self::PublicKey {
        todo!()
    }
}

impl SecretKey for NodeNetworkingSecretKey {
    type PublicKey = NodeNetworkingPublicKey;

    fn sign(&self, _digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        todo!()
    }

    fn to_pk(&self) -> Self::PublicKey {
        todo!()
    }
}

impl SecretKey for ClientSecretKey {
    type PublicKey = ClientPublicKey;

    fn sign(&self, _digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        todo!()
    }

    fn to_pk(&self) -> Self::PublicKey {
        todo!()
    }
}

impl SecretKey for AccountOwnerSecretKey {
    type PublicKey = AccountOwnerPublicKey;

    fn sign(&self, _digest: &[u8; 32]) -> <Self::PublicKey as PublicKey>::Signature {
        todo!()
    }

    fn to_pk(&self) -> Self::PublicKey {
        todo!()
    }
}

impl From<[u8; 32]> for NodeNetworkingPublicKey {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<[u8; 32]> for AccountOwnerPublicKey {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<[u8; 96]> for NodePublicKey {
    fn from(value: [u8; 96]) -> Self {
        Self(value)
    }
}
