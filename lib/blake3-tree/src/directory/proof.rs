use super::{Digest, DirectoryEntry, Link};
use crate::directory::hash::{hash_entry, iv};
use crate::{IncrementalVerifier, IncrementalVerifierError, ProofBuf};

pub enum FindEntryOutput {
    Found(usize, ProofBuf, Link),
    EmptyDirectory,
    NotFoundLeft(ProofBuf, DirectoryEntry),
    NotFoundRight(ProofBuf, DirectoryEntry),
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
                let hash = hash_entry(dbg!(verifier.is_root()), *counter, requested_name, link);
                verifier.verify_hash(&hash)?;
                Ok(())
            },
            FindEntryOutput::EmptyDirectory => todo!(),
            FindEntryOutput::NotFoundLeft(_, _) => todo!(),
            FindEntryOutput::NotFoundRight(_, _) => todo!(),
            FindEntryOutput::NotFound(_, _, _, _, _) => todo!(),
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
