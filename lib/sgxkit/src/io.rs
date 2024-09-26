use std::io::ErrorKind;
use std::sync::atomic::AtomicBool;

use sgxkit_sys::fn0;

use crate::error::HostError;

/// Get the input data string (request).
pub fn get_input_data_string() -> Result<String, std::io::Error> {
    let len = unsafe { fn0::input_data_size() };
    let mut buf = vec![0u8; len as usize];

    if len > 0 {
        // SAFETY: we initialized the string with the length that will be written
        let res = unsafe { fn0::input_data_copy(buf.as_mut_ptr() as usize, 0, len) };
        HostError::result(res)?;
    }

    String::from_utf8(buf).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))
}

static IS_WRITER_OPEN: AtomicBool = AtomicBool::new(false);

/// Writer for output data (response).
pub struct OutputWriter {
    _p: (),
}

impl OutputWriter {
    /// Create an output writer. Only one writer can be created at once.
    #[inline(always)]
    pub fn new() -> Self {
        if !IS_WRITER_OPEN.swap(true, std::sync::atomic::Ordering::Relaxed) {
            OutputWriter { _p: () }
        } else {
            panic!("only one output writer can exist");
        }
    }
}

impl Default for OutputWriter {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OutputWriter {
    fn drop(&mut self) {
        IS_WRITER_OPEN.swap(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl std::io::Write for OutputWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = unsafe { fn0::output_data_append(buf.as_ptr() as usize, buf.len()) };
        HostError::result(res)?;
        Ok(buf.len())
    }

    /// no-op
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
