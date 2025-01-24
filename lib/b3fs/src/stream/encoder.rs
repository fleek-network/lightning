use super::buffer::ProofBuf;
use super::walker::Direction;

/// An encoder that manages a reverse buffer which can be used to convert the
/// root-to-leaf ordering of the [`TreeWalker`] to the proper stack ordering.
///
/// [`TreeWalker`]: crate::walker::TreeWalker
pub struct ProofEncoder {
    cursor: usize,
    size: usize,
    buffer: Box<[u8]>,
}

impl ProofEncoder {
    /// Create a new proof encoder for encoding a tree with the provided max number of
    /// items. An instance of ProofEncoder can not be used to encode more than the `n`
    /// items specified here. Providing an `n` smaller than the actual number of nodes
    /// can result in panics.
    pub fn new(n: usize) -> Self {
        // Compute the byte capacity for this encoder, which is 32-byte per hash and 1
        // byte per 8 one of these.
        let capacity = n * 32 + n.div_ceil(8);
        let mut vec = vec![0; capacity];

        let buffer = vec.into_boxed_slice();
        debug_assert_eq!(
            buffer.len(),
            capacity,
            "The buffer is smaller than expected."
        );

        Self {
            buffer,
            cursor: capacity,
            size: 0,
        }
    }

    /// Insert a new node into the tree, the direction determines whether or not we should
    /// be flipping the stack order when we're trying to rebuild the tree later on (on the
    /// client side).
    ///
    /// # Panics
    ///
    /// If called more than the number of times specified when it got constructed.
    pub fn insert(&mut self, direction: Direction, hash: &[u8; 32]) {
        assert!(self.cursor > 0);

        // Get the current non-finalized sign byte.
        let mut sign = self.buffer[self.cursor - 1];

        self.cursor -= 32;
        self.buffer[self.cursor..self.cursor + 32].copy_from_slice(hash);

        // update the sign byte.
        if direction == Direction::Left {
            sign |= 1 << (self.size & 7);
        }

        self.size += 1;

        // Always put the sign as the leading byte of the data without moving the
        // cursor, this way the finalize can return a valid proof for when it's
        // called when the number of items does not divide 8.
        self.buffer[self.cursor - 1] = sign;

        // If we have consumed a multiple of 8 hashes so far, consume the sign byte
        // by moving the cursor.
        if self.size & 7 == 0 {
            debug_assert!(self.cursor > 0);
            self.cursor -= 1;
            // If we have more data coming in, make sure the dirty byte which will
            // be used for the next sign byte is set to zero.
            if self.cursor > 0 {
                self.buffer[self.cursor - 1] = 0;
            }
        }
    }

    /// Finalize the result of the encoder and return the proof buffer.
    pub fn finalize(mut self) -> ProofBuf {
        // Here we don't want to consume or get a mutable reference to the internal buffer
        // we have, but also we might be called when the number of passed hashes does not
        // divide 8. In this case we already have the current sign byte as the leading byte,
        // so we need to return data start one byte before the cursor.
        //
        // Furthermore we could have been returning a Vec here, but that would imply that the
        // current allocated memory would needed to be copied first into the Vec (in case the
        // cursor != 0) and then freed as well, which is not really suitable for this use case
        // we want to provide the caller with the buffer in the valid range (starting from cursor)
        // and at the same time avoid any memory copy and extra allocation and deallocation which
        // might come with dropping the box and acquiring a vec.
        //
        // This way the caller will have access to the data, and can use it the way they want,
        // for example sending it over the wire, and then once they are done with reading the
        // data they can free the used memory.
        //
        // Another idea here is to also leverage a slab allocator on the Context object which we
        // are gonna have down the line which may improve the performance (not sure how much).
        if self.size & 7 > 0 {
            debug_assert!(self.cursor > 0);
            // shit the final sign byte.
            self.buffer[self.cursor - 1] <<= 8 - (self.size & 7);
            ProofBuf {
                buffer: self.buffer,
                index: self.cursor - 1,
            }
        } else {
            ProofBuf {
                buffer: self.buffer,
                index: self.cursor,
            }
        }
    }
}
