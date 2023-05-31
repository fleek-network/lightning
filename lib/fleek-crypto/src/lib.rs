use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodePublicKey;
pub struct NodeSecretKey;
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeSignature;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingPublicKey;
pub struct NodeNetworkingSecretKey;
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct NodeNetworkingSignature;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientPublicKey;
pub struct ClientSecretKey;
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct ClientSignature;

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct AccountOwnerPublicKey;
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
