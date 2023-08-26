use fxhash::{FxHashMap, FxHashSet};

use crate::frame::{Digest, MessageInternedId};

// TODO: Make this persist.
pub struct Database {
    data: FxHashMap<Digest, Entry>,
}

struct Entry {
    id: MessageInternedId,
    seen: bool,
}

impl Database {
    /// Insert a new message with its interned id to the table.
    pub fn insert(&mut self, id: MessageInternedId, digest: Digest) {
        self.data.insert(digest, Entry { id, seen: false });
    }

    /// Mark a digest as seen.
    pub fn mark_as_seen(&mut self, digest: &Digest) {
        let Some(e) = self.data.get_mut(digest) else {
            return;
        };
        e.seen = true;
    }

    /// Returns the id of the provided digest. Or `None` if the digest
    /// has not been inserted before.
    pub fn get_id_of(&self, digest: &Digest) -> Option<MessageInternedId> {
        self.data.get(digest).map(|e| e.id)
    }

    /// Returns true if we have already seen a digest.
    pub fn have_seen(&self, digest: &Digest) -> bool {
        self.data.get(digest).map(|e| e.seen).unwrap_or(false)
    }
}
