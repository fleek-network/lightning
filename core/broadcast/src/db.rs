use fxhash::FxHashSet;

use crate::frame::Digest;

// TODO: Make this persist.
pub struct Database {
    seen: FxHashSet<Digest>,
}

impl Database {
    /// Returns true if we have already seen a digest.
    pub fn have_seen(&self, digest: &Digest) -> bool {
        self.seen.contains(digest)
    }

    /// Mark a digest as seen.
    pub fn mark_as_seen(&mut self, digest: Digest) {
        self.seen.insert(digest);
    }
}
