/// The public key of a peer.
#[derive(Debug, Hash, Clone, Copy, PartialEq, PartialOrd)]
pub enum PeerId {
    Ed25519([u8; 32]),
    BLS([u8; 48]),
}

/// A signature.
#[derive(Debug, Hash, Clone, Copy, PartialEq, PartialOrd)]
pub enum Signature {
    Ed25519([u8; 64]),
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
            if !Self::verify_signature(*digest, *signer, *signature) {
                return false;
            }
        }

        true
    }
}
