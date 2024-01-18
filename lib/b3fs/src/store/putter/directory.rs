use super::error::PutFeedProofError;
use crate::directory::{DirectoryHasher, OwnedEntry};
use crate::verifier::{IncrementalVerifier};

pub struct DirectoryPutter {
    expected_hash: Option<[u8; 32]>,
    entries: Vec<OwnedEntry>,
    mode: Mode,
}

enum Mode {
    WithVerifier(IncrementalVerifier),
    WithHasher(DirectoryHasher),
}

impl DirectoryPutter {
    pub fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError> {
        match &mut self.mode {
            Mode::WithVerifier(verifier) => verifier.feed_proof(proof).map_err(|e| e.into()),
            Mode::WithHasher(_) if self.entries.len() != 0 || self.expected_hash.is_none() => {
                return Err(PutFeedProofError::UnexpectedCall);
            },
            Mode::WithHasher(_) => {
                let mut verifier = IncrementalVerifier::dir(self.expected_hash.unwrap());
                let out = verifier.feed_proof(proof).map_err(|e| e.into());
                self.mode = Mode::WithVerifier(verifier);
                out
            },
        }
    }

    pub fn insert(&mut self, entry: OwnedEntry) {
        match self.mode {
            Mode::WithVerifier(_) => todo!(),
            Mode::WithHasher(_) => todo!(),
        }
        self.entries.push(entry);
    }
}
