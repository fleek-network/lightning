pub trait TreeCapture {
    /// Push the given hash to the tree capture API.
    fn push(&mut self, hash: [u8; 32]);
}
