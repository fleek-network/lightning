//! Error types

use std::fmt::Display;

use strum::EnumMessage;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumMessage, strum::FromRepr)]
#[repr(i32)]
pub enum HostError {
    /// Insufficient balance for call
    InsufficientBalance = -1,

    /// Specified pointers were out of bounds
    OutOfBounds = -2,

    /// Invalid key derivation path
    KeyDerivationInvalidPath = -3,
    /// Key derivation error
    KeyDerivation = -4,

    /// Invalid permission header for shared key
    UnsealInvalidPermissionHeader = -5,
    /// Current wasm is not approved to access global content
    UnsealPermissionDenied = -6,
    /// Sealed data could not be decrypted
    UnsealFailed = -7,

    /// Unexpected error
    #[default]
    Unexpected = -99,
}

impl HostError {
    // Convert a response int into a result, where any value less than
    // zero is a host error.
    #[must_use = "host errors should be used"]
    pub fn result(value: i32) -> Result<i32, Self> {
        match HostError::from_repr(value) {
            None => Ok(value),
            Some(err) => Err(err),
        }
    }
}

impl std::error::Error for HostError {}

impl Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_documentation().unwrap())
    }
}

impl From<HostError> for std::io::Error {
    fn from(value: HostError) -> Self {
        let kind = match value {
            HostError::KeyDerivationInvalidPath => std::io::ErrorKind::InvalidInput,
            HostError::UnsealInvalidPermissionHeader => std::io::ErrorKind::InvalidData,
            HostError::UnsealPermissionDenied => std::io::ErrorKind::PermissionDenied,
            HostError::InsufficientBalance
            | HostError::OutOfBounds
            | HostError::KeyDerivation
            | HostError::UnsealFailed
            | HostError::Unexpected => std::io::ErrorKind::Other,
        };
        std::io::Error::new(kind, value.to_string())
    }
}
