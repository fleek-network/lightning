use lightning_interfaces::types::Digest;
use quick_cache::unsync::Cache;
use tracing::error;

use crate::frame::MessageInternedId;
use crate::Message;

// TODO: Make this persist.
pub struct Database {
    data: Cache<Digest, Entry>,
}

struct Entry {
    id: MessageInternedId,
    message: Option<Message>,
    propagated: bool,
}

impl Database {
    pub fn new() -> Self {
        // todo(dalton): Figure out a sane cache for broadcast messaging. Ideally a 24 hour epoch
        // worth.
        Self {
            data: Cache::new(100_000),
        }
    }
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

    /// Takes &mut self and uses get_mut() for performance. We save some atomic operations in
    /// quick_cache and we are returning a clone anyway
    pub fn get_message(&mut self, digest: &Digest) -> Option<Message> {
        self.data.get_mut(digest).and_then(|e| e.message.clone())
    }

    /// Takes &mut self and uses get_mut() for performance. We save some atomic operations in
    /// quick_cache and we are returning a clone anyway
    pub fn get_id(&mut self, digest: &Digest) -> Option<MessageInternedId> {
        // We use get_mut here because we are cloning anyway and it is more effecient for
        // quick_cache
        self.data.get_mut(digest).map(|e| e.id)
    }

    /// Returns true if we have already propagated a message. Which is the final state a message
    /// could be in. The highest level of *having seen a message*.
    /// takes &mut self so we can use get_mut() on our cache which is more performant
    pub fn is_propagated(&mut self, digest: &Digest) -> bool {
        self.data
            .get_mut(digest)
            .map(|e| e.propagated)
            .unwrap_or(false)
    }

    /// Mark the message known with the given digest as propagated.
    pub fn mark_propagated(&mut self, digest: &Digest) {
        let Some(mut e) = self.data.get_mut(digest) else {
            debug_assert!(false, "We should not mark a message we haven't seen as propagated.");
            return;
        };
        e.propagated = true;
    }

    /// Insert the payload of a message known with the given digest.
    pub fn insert_message(&mut self, digest: &Digest, message: Message) {
        let Some(mut e) = self.data.get_mut(digest) else {
            error!("This should never happen...call Dalton");
            debug_assert!(false, "We should not be inserting payload of what we have not seen.");
            return;
        };
        e.message = Some(message);
    }
}

#[cfg(test)]
mod test {
    use fleek_crypto::NodeSignature;
    use ink_quill::ToDigest;
    use lightning_interfaces::types::Topic;

    use super::*;

    #[test]
    fn test_cache() {
        let mut db = Database::new();
        // We will set this to the sixty thousand entry we make into the cache
        let mut sixty_thousand = None;
        // we will set this to the first entry we put in the cache
        let mut first = None;

        for i in 0..120_000u32 {
            // Build a message to insert, use the index to make unique payload/digest
            let message = Message {
                origin: 0,
                signature: NodeSignature([0; 64]),
                topic: Topic::Consensus,
                timestamp: 0,
                payload: i.to_le_bytes().into(),
            };
            let digest = message.to_digest();

            // Save the first and sixtieth entry to check against later
            if i == 0 {
                first = Some((digest, message.clone()));
            } else if i == 59_999 {
                sixty_thousand = Some((digest, message.clone()));
            }

            db.insert_with_message(0, digest, message);
            // Read it once each so it evicts in a predictable order
            db.get_message(&digest);
        }
        let sixty_thousand = sixty_thousand.unwrap();
        let first = first.unwrap();

        // Check that the cache is full
        assert_eq!(db.data.capacity() as usize, db.data.len());

        // Get the 60_000 message from the DB this should still be cached
        let sixty_thou_msg = db.get_message(&sixty_thousand.0);
        assert_eq!(sixty_thou_msg.unwrap().payload, sixty_thousand.1.payload);

        // Get the first message it should have been evicted from the cache
        let first_msg = db.get_message(&first.0);
        assert!(first_msg.is_none());
    }
}
