use std::ops::{Deref, DerefMut};

/// A wrapper around a Vec that has a cheaper clear operation in exchange for added overhead on
/// drops
pub struct Buffer(Vec<u8>);

impl Buffer {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Sets the length of the buffer to 0, but does not deallocate the buffer nor drop any elements
    pub fn clear(&mut self) {
        // saftey: drop function ensures entire buffer is dropped
        unsafe { self.0.set_len(0) }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::io::Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Deref for Buffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // safety: we know that the buffer is allocated
        unsafe { self.0.set_len(self.0.capacity()) }
    }
}
