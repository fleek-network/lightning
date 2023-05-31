use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// The public key of a peer.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
#[deprecated = "Replace with `fleek_crypto::TransactionSender`."]
pub enum PeerId {
    Ed25519([u8; 32]),
    #[serde(with = "BigArray")]
    BLS([u8; 48]),
}

/// A signature.
#[derive(Debug, Hash, Clone, Copy, PartialEq, PartialOrd, Eq, Serialize, Deserialize)]
#[deprecated = "Replace with `fleek_crypto::TransactionSignature`."]
pub enum Signature {
    #[serde(with = "BigArray")]
    Ed25519([u8; 64]),
    #[serde(with = "BigArray")]
    BLS([u8; 96]),
}
