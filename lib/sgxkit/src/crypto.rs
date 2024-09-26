use sgxkit_sys::fn0;

use crate::error::HostError;

/// Derived wasm module specific key. Full path can be at maximum [u16; 128].
///
/// Secret material is never available directly, but rather supports methods
/// for deriving children, signing hashes of data, and unsealing encrypted data.
///
/// ```
/// use sgxkit::DerivedKey;
///
/// let key = DerivedKey::new(b"")
/// ```
pub struct DerivedKey {
    path: Vec<u16>,
}

impl DerivedKey {
    /// Create the root wasm derived key
    #[inline(always)]
    pub fn root() -> Self {
        Self { path: vec![] }
    }

    /// Create a new derived key, with an optional derivation path.
    /// Returns `None` if path is invalid.
    #[inline(always)]
    pub fn new(path: &[u16]) -> Option<Self> {
        (path.len() <= 128).then_some(Self {
            path: path.to_vec(),
        })
    }

    /// Create a new derived key from a slice. Must have an even number of bytes <= 256 present
    #[inline(always)]
    pub fn from_slice(path: &[u8]) -> Option<Self> {
        (path.len() <= 256 && path.len() & 2 == 0).then(|| {
            let path = path
                .chunks(2)
                .map(|v| u16::from_be_bytes(v.try_into().unwrap()))
                .collect::<Vec<_>>();
            Self { path }
        })
    }

    /// Derive a child path from the key
    pub fn with_child_path(mut self, child_path: &[u16]) -> Option<Self> {
        let len = self.path.len() + child_path.len();
        if len <= 128 {
            self.path.extend_from_slice(child_path);
            Some(self)
        } else {
            None
        }
    }

    /// Sign the sha256 digest of some given data using the derived key.
    ///
    /// This can be used as a proof
    pub fn sign(&self, data: &[u8]) -> Result<[u8; 65], HostError> {
        let mut buf = [0u8; 65];

        let res = unsafe {
            fn0::derived_key_sign(
                self.path.as_ptr() as usize,
                self.path.len(),
                data.as_ptr() as usize,
                data.len(),
                buf.as_mut_ptr() as usize,
            )
        };
        HostError::result(res)?;

        Ok(buf)
    }

    /// Unseal a ciphertext in-place using the derived key.
    ///
    /// Unlike [`shared_key_unseal`], derived keys can only be accessed
    /// by the wasm module itself and as such, no permission header is
    /// required to access data encrypted for these keys.
    pub fn unseal(&self, cipher: &mut Vec<u8>) -> Result<(), HostError> {
        let res = unsafe {
            fn0::derived_key_unseal(
                self.path.as_ptr() as usize,
                self.path.len(),
                cipher.as_mut_ptr() as usize,
                cipher.len(),
            )
        };
        let len = HostError::result(res)?;

        // Truncate the (previously) cipher data to the written plaintext length
        cipher.truncate(len as usize);

        Ok(())
    }
}

/// Unseal data in-place using the network's shared extended key.
///
/// Accepts a mutable vector of sealed data, and decrypts it in-place.
/// Encrypted data length must not exceed [`i32::MAX`] (roughly 2GiB).
///
/// Encrypted content must contain a permissions header with a list of approved wasm hashes that
/// can access the data, formatted as follows:
///
/// ```text
/// [ b"FLEEK_ENCLAVE_APPROVED_WASM" . num hashes (u8) . hash one (32 bytes) ... content (unsized) ]
/// ```
pub fn shared_key_unseal(data: &mut Vec<u8>) -> Result<(), HostError> {
    // Unseal the data
    let res = unsafe { fn0::shared_key_unseal(data.as_mut_ptr() as usize, data.len()) };
    let len = HostError::result(res)?;

    // Truncate the (previously) cipher data into the written plaintext length
    data.truncate(len as usize);

    Ok(())
}
