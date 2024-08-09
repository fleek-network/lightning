use std::io::ErrorKind;
use std::sync::atomic::AtomicBool;

use sgxkit_sys::fn0;

/// Get the input data string (request).
pub fn get_input_data() -> Option<String> {
    unsafe {
        let len = fn0::input_data_size();
        (len > 0).then(|| {
            let buf = String::with_capacity(len as usize);
            fn0::input_data_copy(buf.as_ptr() as usize, 0, len);
            buf
        })
    }
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
        let res = unsafe { fn0::output_data_append(buf.as_ptr() as usize, buf.len() as u32) };
        match res {
            0 => Ok(buf.len()),
            -1 => Err(std::io::Error::new(
                ErrorKind::Other,
                "memory export not found",
            )),
            -2 => Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "out of bounds",
            )),
            -3 => Err(std::io::Error::new(ErrorKind::Other, "unexpected error")),
            _ => unreachable!("unknown response code"),
        }
    }

    /// no-op
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
