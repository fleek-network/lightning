#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IOError {
    MemoryNotFound = -1,
    OutOfBounds = -2,
    Unknown = -3,
}

impl IOError {
    pub fn result(value: i32) -> Result<(), Self> {
        match value {
            0 => Ok(()),
            -1 => Err(IOError::MemoryNotFound),
            -2 => Err(IOError::OutOfBounds),
            _ => Err(IOError::Unknown),
        }
    }
}
