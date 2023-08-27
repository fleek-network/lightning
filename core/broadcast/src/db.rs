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
    /// Insert a message with all the information we have about it.
    pub fn insert_with_message(&mut self, id: MessageInternedId, digest: Digest, message: Message) {
        self.data.insert(
            digest,
            Entry {
                id,
                message: Some(message),
            },
        );
    }

    pub fn get_message(&self, digest: &Digest) -> Option<Message> {
        self.data.get(digest).and_then(|e| e.message.clone())
    }

    pub fn get_id(&self, digest: &Digest) -> Option<MessageInternedId> {
        self.data.get(digest).map(|e| e.id)
    }
}
