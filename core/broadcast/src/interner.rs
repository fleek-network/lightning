use lightning_interfaces::types::Digest;

use crate::frame::MessageInternedId;

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
    pub const MAX_CAPACITY: usize = 65_536;

    /// Create a new interner with the provided capacity. In practice consider
    /// passing [`Interner::MAX_CAPACITY`] as the capacity. That will only
    /// consume 2MB of memory.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity <= Self::MAX_CAPACITY);
        Self {
            next: 0,
            data: Vec::with_capacity(capacity),
        }
    }

    /// Insert the given digest to the interning table and returns the assigned id.
    #[inline(always)]
    pub fn insert(&mut self, digest: Digest) -> MessageInternedId {
        let index = self.next;
        self.next = index.wrapping_add(1);

        if self.data.len() < Self::MAX_CAPACITY {
            self.data.push(digest);
        } else {
            self.data[index as usize] = digest;
        }

        index
    }

    /// Return the digest known by this id, or `None` if we have not reached that
    /// id yet.
    #[inline(always)]
    pub fn get(&self, id: MessageInternedId) -> Option<&Digest> {
        self.data.get(id as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_digest(index: u16, fill: u8) -> Digest {
        let mut value = [fill; 32];
        value[0..2].copy_from_slice(&index.to_le_bytes());
        value
    }

    #[test]
    fn interner() {
        let mut interner = Interner::new(Interner::MAX_CAPACITY);

        for i in 0..=u16::MAX {
            let id = interner.insert(to_digest(i, 0));
            assert_eq!(id, i);
            assert_eq!(interner.get(id), Some(&to_digest(i, 0)));
        }

        assert_eq!(interner.data.len(), Interner::MAX_CAPACITY);

        for i in 0..u16::MAX {
            if i == 100 {
                for id in 100..200 {
                    assert_eq!(interner.get(id), Some(&to_digest(id, 0)));
                }
            }

            let id = interner.insert(to_digest(i, 1));
            assert_eq!(id, i);
            assert_eq!(interner.get(id), Some(&to_digest(i, 1)));
        }

        assert_eq!(interner.data.len(), Interner::MAX_CAPACITY);
    }
}
