//! Time related utilities

use sgxkit_sys::fn0;

/// Get an insecure timestamp from the host os, outside of SGX.
///
/// This function should *NEVER* be used for cryptographic purposes,
/// such as validating certificates, nonces, etc. The time is obtained
/// from the host os, and is subject to modification. This operation is
/// *NOT* gauranteed to be monotonic, so subsequent calls may return a
/// smaller value than the previous call.
///
/// # Returns
///
/// The current u64 timestamp in seconds
pub fn insecure_systemtime() -> u64 {
    unsafe { fn0::insecure_systemtime() }
}
