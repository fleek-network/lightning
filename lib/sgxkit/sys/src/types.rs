use std::io::ErrorKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IOError {
    MemoryNotFound = -1,
    OutOfBounds = -2,
    Unknown = -3,
}

impl IOError {
    pub fn result(value: i32) -> Result<(), Self> {
        match IOError::try_from(value) {
            Ok(err) => Err(err),
            Err(()) => Ok(()),
        }
    }
}

impl TryFrom<i32> for IOError {
    type Error = ();
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Err(()),
            -1 => Ok(IOError::MemoryNotFound),
            -2 => Ok(IOError::OutOfBounds),
            _ => Ok(IOError::Unknown),
        }
    }
}

impl From<IOError> for std::io::Error {
    fn from(value: IOError) -> Self {
        match value {
            IOError::MemoryNotFound => {
                std::io::Error::new(ErrorKind::Other, "memory export not found")
            },
            IOError::OutOfBounds => std::io::Error::new(ErrorKind::InvalidInput, "out of bounds"),
            IOError::Unknown => std::io::Error::new(ErrorKind::Other, "unexpected error"),
        }
    }
}
