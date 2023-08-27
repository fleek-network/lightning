use fxhash::{FxHashMap, FxHashSet};

use crate::frame::{Digest, MessageInternedId};
use crate::Message;

// TODO: Make this persist.
#[derive(Default)]
pub struct Database {
    data: FxHashMap<Digest, Entry>,
}

struct Entry {
    id: MessageInternedId,
    message: Option<Message>,
}

impl Database {
    pub fn get_message(&self, digest: &Digest) -> Option<Message> {
        self.data.get(digest).map(|e| e.message.clone()).flatten()
    }
}
