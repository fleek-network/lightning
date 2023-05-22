use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// The public key of a peer.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum PeerId {
    Ed25519([u8; 32]),
    #[serde(with = "BigArray")]
    BLS([u8; 48]),
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct BlsPublicKey(#[serde(with = "BigArray")] pub [u8; 48]);

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Ed25519PublicKey(pub [u8; 32]);

/// A signature.
#[derive(Debug, Hash, Clone, Copy, PartialEq, PartialOrd, Eq, Serialize, Deserialize)]
pub enum Signature {
    #[serde(with = "BigArray")]
    Ed25519([u8; 64]),
    #[serde(with = "BigArray")]
    BLS([u8; 96]),
}

/// A backend implementing functionality for verifying a signature.
pub trait SignatureVerifierInterface {
    /// Returns [`true`] if the provided signature from the provided signer is valid, returns
    /// false otherwise.
    ///
    /// # Panics
    ///
    /// This function must not panic.
    fn verify_signature(digest: &[u8; 32], signer: &PeerId, signature: &Signature) -> bool;

    /// Returns [`true`] if all of the provided signatures are valid.
    ///
    /// # Default implementation
    ///
    /// The default implementation performs a sequential check one by one, which is correct,
    /// but if verifying the check in batch is more optimal.
    ///
    /// # Panics
    ///
    /// This function should panic if the size of the provided inputs is not the same thing,
    /// the caller must ensure that the same number of `digest`, `signer` and `signature` is
    /// provided.
    fn verify_signature_batch(
        digest: &[&[u8; 32]],
        signer: &[&PeerId],
        signature: &[&Signature],
    ) -> bool {
        assert_eq!(digest.len(), signer.len());
        assert_eq!(digest.len(), signature.len());

        for (digest, (signer, signature)) in digest.iter().zip(signer.iter().zip(signature)) {
            if !Self::verify_signature(digest, signer, signature) {
                return false;
            }
        }

        true
    }
}

impl From<BlsPublicKey> for PeerId {
    fn from(value: BlsPublicKey) -> Self {
        Self::BLS(value.0)
    }
}

impl From<Ed25519PublicKey> for PeerId {
    fn from(value: Ed25519PublicKey) -> Self {
        Self::Ed25519(value.0)
    }
}
