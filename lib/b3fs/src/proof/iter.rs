use crate::utils::is_valid_proof_len;

const SEGMENT_SIZE: usize = 32 * 8 + 1;

/// An iterator over a proof buffer which iterates through a packed sign-prepended buffer yielding
/// the `should flip` sign along with the hash at each iteration.
pub struct ProofBufIter<'b> {
    buffer: &'b [u8],
    segment: &'b [u8],
    sign: u8,
}

/// The should flip bit sign in a proof indicates if a hash appears to the left of the last pushed
/// node in the stack.
///
/// For example imagine we have a tree `Root { L, R }`
pub type ShouldFlip = bool;

impl<'b> ProofBufIter<'b> {
    /// Create a new iterator over the provided proof buffer.
    ///
    /// # Panics
    ///
    /// This method panics if the provided proof buffer's length is not valid. You can check the
    /// validity of a proof length by using [`is_valid_proof_len`].
    pub fn new(buffer: &'b [u8]) -> Self {
        if !is_valid_proof_len(buffer.len()) {
            panic!("Invalid proof length.");
        }

        if buffer.is_empty() {
            return Self {
                buffer,
                segment: buffer,
                sign: 0,
            };
        }

        // Number of bytes to read per iteration. For the first iteration read the partial first
        // segment and we will then start reading full segments.
        let mut read = buffer.len() % SEGMENT_SIZE;
        // Handle the case where we have complete segments.
        if read == 0 {
            read = SEGMENT_SIZE;
        }

        let sign = buffer[0];
        let segment = &buffer[1..read];
        let buffer = &buffer[read..];

        Self {
            buffer,
            segment,
            sign,
        }
    }

    /// Returns true if the iterator is over.
    pub fn is_done(&self) -> bool {
        self.segment.is_empty()
    }
}

impl<'b> Iterator for ProofBufIter<'b> {
    type Item = (ShouldFlip, &'b [u8; 32]);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.segment.is_empty() {
            return None;
        }

        let hash = arrayref::array_ref![self.segment, 0, 32];
        let should_flip = (self.sign & 128) != 0;
        self.segment = &self.segment[32..];
        self.sign <<= 1;

        if self.segment.is_empty() && !self.buffer.is_empty() {
            debug_assert!(self.buffer.len() >= SEGMENT_SIZE);
            self.sign = self.buffer[0];
            self.segment = &self.buffer[1..SEGMENT_SIZE];
            self.buffer = &self.buffer[SEGMENT_SIZE..];
        }

        Some((should_flip, hash))
    }
}
