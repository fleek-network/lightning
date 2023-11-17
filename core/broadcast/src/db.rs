use fxhash::FxHashMap;
use lightning_interfaces::types::Digest;

use crate::frame::MessageInternedId;
use crate::Message;

// TODO: Make this persist.
#[derive(Default)]
pub struct Database {
    data: FxHashMap<Digest, Entry>,
}

struct Entry {
    id: MessageInternedId,
    message: Option<Message>,
    propagated: bool,
}

impl Database {
    /// Insert a message with all the information we have about it. And set propagated to true.
    pub fn insert_with_message(&mut self, id: MessageInternedId, digest: Digest, message: Message) {
        self.data.insert(
            digest,
            Entry {
                id,
                message: Some(message),
                propagated: true,
            },
        );
    }

    /// Insert the id for a digest.
    pub fn insert_id(&mut self, id: MessageInternedId, digest: Digest) {
        self.data.insert(
            digest,
            Entry {
                id,
                message: None,
                propagated: false,
            },
        );
    }

    pub fn get_message(&self, digest: &Digest) -> Option<Message> {
        self.data.get(digest).and_then(|e| e.message.clone())
    }

    pub fn get_id(&self, digest: &Digest) -> Option<MessageInternedId> {
        self.data.get(digest).map(|e| e.id)
    }

    /// Returns true if we have already propagated a message. Which is the final state a message
    /// could be in. The highest level of *having seen a message*.
    pub fn is_propagated(&self, digest: &Digest) -> bool {
        self.data.get(digest).map(|e| e.propagated).unwrap_or(false)
    }

    /// Mark the message known with the given digest as propagated.
    pub fn mark_propagated(&mut self, digest: &Digest) {
        let Some(e) = self.data.get_mut(digest) else {
            debug_assert!(false, "We should not mark a message we haven't seen as propagated.");
            return;
        };
        e.propagated = true;
    }

    /// Insert the payload of a message known with the given digest.
    pub fn insert_message(&mut self, digest: &Digest, message: Message) {
        let Some(e) = self.data.get_mut(digest) else {
            debug_assert!(false, "We should not be inserting payload of what we have not seen.");
            return;
        };
        e.message = Some(message);
    }
}
