use super::hash::EMPTY_HASH;
use super::{Digest, DirectoryEntry, Link};
use crate::directory::hash::{hash_entry, iv};
use crate::{IncrementalVerifier, IncrementalVerifierError, ProofBuf};

pub enum FindEntryOutput {
    Found(usize, ProofBuf, Link),
    EmptyDirectory,
    NotFoundLeft(ProofBuf, DirectoryEntry),
    NotFoundRight(usize, ProofBuf, DirectoryEntry),
    NotFound(usize, ProofBuf, DirectoryEntry, ProofBuf, DirectoryEntry),
}

impl FindEntryOutput {
    /// Returns `true` if this output indicates that the requested directory entry was found.
    #[inline]
    pub fn is_found(&self) -> bool {
        matches!(self, FindEntryOutput::Found(_, _, _))
    }

    /// If the output is a `Found` returns the link.
    #[inline]
    pub fn get_link(&self) -> Option<&Link> {
        if let FindEntryOutput::Found(_, _, link) = &self {
            Some(link)
        } else {
            None
        }
    }

    pub fn verify(
        &self,
        root_hash: Digest,
        requested_name: &str,
    ) -> Result<(), IncrementalVerifierError> {
        match self {
            FindEntryOutput::Found(counter, proof, link) => {
                let mut verifier = IncrementalVerifier::new(root_hash, *counter);
                verifier.set_iv(iv());
                verifier.feed_proof(proof.as_slice())?;
                let hash = hash_entry(verifier.is_root(), *counter, requested_name, link);
                verifier.verify_hash(&hash)?;
                Ok(())
            },
            FindEntryOutput::EmptyDirectory if root_hash == EMPTY_HASH => Ok(()),
            FindEntryOutput::EmptyDirectory => Err(IncrementalVerifierError::HashMismatch),
            FindEntryOutput::NotFoundLeft(_, entry) if entry.name() <= requested_name => {
                Err(IncrementalVerifierError::InvalidRange)
            },
            FindEntryOutput::NotFoundLeft(proof, entry) => {
                let mut verifier = IncrementalVerifier::new(root_hash, 0);
                verifier.set_iv(iv());
                verifier.feed_proof(proof.as_slice())?;
                let hash = hash_entry(verifier.is_root(), 0, entry.name(), entry.link());
                verifier.verify_hash(&hash)?;
                Ok(())
            },
            FindEntryOutput::NotFoundRight(_, _, entry) if entry.name() >= requested_name => {
                Err(IncrementalVerifierError::InvalidRange)
            },
            FindEntryOutput::NotFoundRight(counter, proof, entry) => {
                let mut verifier = IncrementalVerifier::new(root_hash, *counter);
                verifier.set_iv(iv());
                verifier.feed_proof(proof.as_slice())?;
                if !verifier.is_done() {
                    return Err(IncrementalVerifierError::InvalidProofSize);
                }
                let hash = hash_entry(verifier.is_root(), *counter, entry.name(), entry.link());
                verifier.verify_hash(&hash)?;
                Ok(())
            },
            FindEntryOutput::NotFound(_, _, left, _, right)
                if left.name() >= requested_name || right.name() <= requested_name =>
            {
                Err(IncrementalVerifierError::InvalidRange)
            },
            FindEntryOutput::NotFound(0, _, _, _, _)
            | FindEntryOutput::NotFound(usize::MAX, _, _, _, _) => {
                Err(IncrementalVerifierError::InvalidCounter)
            },
            FindEntryOutput::NotFound(mid, left_proof, left_entry, right_proof, right_entry) => {
                let mut verifier = IncrementalVerifier::new(root_hash, mid - 1);
                verifier.set_iv(iv());
                verifier.feed_proof(left_proof.as_slice())?;
                let hash = hash_entry(
                    verifier.is_root(),
                    mid - 1,
                    left_entry.name(),
                    left_entry.link(),
                );
                verifier.verify_hash(&hash)?;
                verifier.feed_proof(right_proof.as_slice())?;
                let hash = hash_entry(
                    verifier.is_root(),
                    mid + 1,
                    right_entry.name(),
                    right_entry.link(),
                );
                verifier.verify_hash(&hash)?;
                Ok(())
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::directory::test_utils::*;

    #[test]
    fn valid_found_proof_and_verify() {
        let dir = mkdir(0..1);
        let output = dir.find_entry(name(0));
        assert_eq!(output.verify(*dir.root_hash(), name(0)), Ok(()));

        let dir = mkdir(0..3);
        let output = dir.find_entry(name(0));
        assert_eq!(output.verify(*dir.root_hash(), name(0)), Ok(()));

        let dir = mkdir(0..5);
        let output = dir.find_entry(name(0));
        assert_eq!(output.verify(*dir.root_hash(), name(0)), Ok(()));
    }
}
