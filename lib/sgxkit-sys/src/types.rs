#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IOError {
    MemoryNotFound = -1,
    OutOfBounds = -2,
    Unknown = -3,
}
