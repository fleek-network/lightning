use std::fmt::{Debug, Display};
use std::ops::Deref;

use super::iter::ProofBufIter;
use crate::utils::{is_valid_proof_len, to_hex};

/// A pretty printer for a proof buffer that uses the proof iterator to unpack and display each
/// hash inside the buffer along with its 'sign'.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofBufPrettyPrinter<'b>(pub(super) &'b [u8]);

impl<'b> ProofBufPrettyPrinter<'b> {
    pub fn new(buffer: &'b [u8]) -> Self {
        assert!(is_valid_proof_len(buffer.len()));
        Self(buffer)
    }
}

impl Debug for ProofBufPrettyPrinter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let iter = ProofBufIter::new(self.0);
        let mut fmt = f.debug_list();
        for (flip, hash) in iter {
            let item = Item(flip, hash);
            fmt.entry(&item);
        }
        fmt.finish()
    }
}

impl Display for ProofBufPrettyPrinter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

struct Item<'h>(bool, &'h [u8; 32]);

impl Debug for Item<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 {
            write!(f, "-{}", to_hex(self.1))
        } else {
            write!(f, "+{}", to_hex(self.1))
        }
    }
}

impl Deref for ProofBufPrettyPrinter<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
