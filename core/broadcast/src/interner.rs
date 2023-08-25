

use crate::frame::{Digest, MessageInternedId};

/// The interner is responsible for assigning a numeric value to each digest
/// that we see. This interning is per execution and there is no guarantee that
/// we keep the same interning on connection resets.
///
/// Although the type used for message interns id is quite large enough, it is
/// still very finite and is limited to 65536 messages. In this implementation
/// we wrap around u16::MAX and go back to index 0 when that happens and we remove
/// the old option.
///
/// This can cause a problem if we advertise a message using a certain id, and
/// get a `WANT(id)` request after we have already advertised 65k other messages.
///
/// This case is unlikely and we do not want to support it anyway.
pub struct Interner {
    next: u16,
    data: Vec<Digest>,
}

impl Interner {
    /// Create a new interner with the provided capacity. In practice consider
    /// passing `u16::MAX` as the capacity. That will only consume 2MB of memory.
    pub fn new(capacity: u16) -> Self {
        Self {
            next: 0,
            data: Vec::with_capacity(capacity.into()),
        }
    }

    /// Insert the given digest to the interning table and returns the assigned id.
    #[inline(always)]
    pub fn insert(&mut self, digest: Digest) -> MessageInternedId {
        let index = self.next;
        self.next = self.next.wrapping_add(1);
        self.data.insert(index as usize, digest);
        index
    }

    /// Return the digest known by this id, or `None` if we have not reached that
    /// id yet.
    #[inline(always)]
    pub fn get(&self, id: MessageInternedId) -> Option<&Digest> {
        self.data.get(id as usize)
    }
}
