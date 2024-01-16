use std::collections::VecDeque;

/// The tree keeper can be used to re-generate a full hash tree from an stream of proofs.
#[derive(Default)]
pub struct TreeKeeper {
    counter: usize,
    queue: VecDeque<[u8; 32]>,
    tree: Vec<[u8; 32]>,
}

impl TreeKeeper {
    /// Push a new hash into the parent queue.
    pub fn push(&mut self, hash: [u8; 32]) {
        self.queue.push_front(hash);
    }

    /// Pop an element from the queue, can be used to revert the state changes from pushes.
    pub fn pop(&mut self) {
        self.queue.pop_front().expect("Queue underflow.");
    }

    /// Advance the counter forward. Should be called after a content is validated.
    pub fn advance(&mut self) {
        self.counter += 1;
        let mut total_entries = self.counter;
        while (total_entries & 1) == 0 {
            total_entries >>= 1;
            let hash = self.queue.pop_back().unwrap();
            self.tree.push(hash);
        }
    }

    /// Finalize the tree and return the generated tree.
    pub fn take(mut self) -> Vec<[u8; 32]> {
        while let Some(hash) = self.queue.pop_back() {
            self.tree.push(hash);
        }
        self.tree
    }
}
