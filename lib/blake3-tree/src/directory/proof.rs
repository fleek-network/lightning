use thiserror::Error;

use super::{DirectoryEntry, Link};
use crate::ProofBuf;

pub enum FindEntryOutput {
    Found(ProofBuf, Link),
    EmptyDirectory,
    NotFoundLeft(ProofBuf, DirectoryEntry),
    NotFoundRight(ProofBuf, DirectoryEntry),
    NotFound(usize, ProofBuf, DirectoryEntry, ProofBuf, DirectoryEntry),
}

#[derive(Error, Debug)]
#[error("Invalid FindEntryOutput")]
pub struct InvalidProof;

impl FindEntryOutput {
    /// Returns `true` if this output indicates that the requested directory entry was found.
    #[inline]
    pub fn is_found(&self) -> bool {
        matches!(self, FindEntryOutput::Found(_, _))
    }

    /// If the output is a `Found` returns the link.
    #[inline]
    pub fn get_link(&self) -> Option<&Link> {
        if let FindEntryOutput::Found(_, link) = &self {
            Some(link)
        } else {
            None
        }
    }

    pub fn verify(&self, _requested_name: &str) -> Result<(), InvalidProof> {
        todo!()
    }
}
