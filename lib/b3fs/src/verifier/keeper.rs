use std::collections::VecDeque;

use crate::proof::iter::ProofBufIter;

/// The tree keeper can be used to re-generate a full hash tree from an stream of proofs.
#[derive(Default)]
pub struct TreeKeeper {
    counter: usize,
    queue: VecDeque<[u8; 32]>,
    tree: Vec<[u8; 32]>,
}

impl TreeKeeper {
    pub fn push_parent(&mut self) {
        
    }
}
