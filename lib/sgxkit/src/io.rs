//! IO related utilites

use std::io::{Read, Seek};
use std::sync::{Mutex, MutexGuard};

use sgxkit_sys::fn0;

use crate::error::HostError;

/// Print formatted text to debug log (if enabled on the node).
#[macro_export]
macro_rules! println {
    () => {
        #[cfg(target_family = "wasm")]
        $crate::io::debug_print('\n');

        #[cfg(not(target_family = "wasm"))]
        std::println!();
    };
    ($($arg:tt)*) => {
        #[cfg(target_family = "wasm")]
        $crate::io::debug_print(format!($($arg)*));

        #[cfg(not(target_family = "wasm"))]
        std::println!($($arg)*);
    };
}

/// Print string to debug log (if enabled on the node).
#[inline(always)]
pub fn debug_print<S: AsRef<str>>(msg: S) {
    let s = msg.as_ref();
    unsafe { fn0::debug_print(s.as_ptr() as usize, s.len()) }
}

/// Construct a new reader for the input data.
#[inline(always)]
pub fn input_reader() -> InputReader {
    let len = unsafe { fn0::input_data_size() };
    InputReader { cur: 0, len }
}

/// Shorthand to read the input data as a string.
#[inline(always)]
pub fn get_input_data_string() -> Result<String, std::io::Error> {
    let mut input = input_reader();
    let mut string = String::with_capacity(input.len());
    input.read_to_string(&mut string)?;
    Ok(string)
}

pub struct InputReader {
    cur: usize,
    len: usize,
}

impl InputReader {
    // Get the overall length of the input data
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    // Shorthand to check if the input data is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Read for InputReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Early return for end of stream
        if self.cur == self.len {
            return Ok(0);
        }

        // Compute the amount of bytes to write to the buffer
        let size = buf.len().min(self.len - self.cur);

        // Copy data from input source into buffer
        let res = unsafe { fn0::input_data_copy(buf.as_mut_ptr() as usize, self.cur, size) };
        HostError::result(res)?;

        self.cur += size;

        Ok(size)
    }
}

impl Seek for InputReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(v) => self.cur = v.try_into().unwrap(),
            std::io::SeekFrom::End(v) => {
                let pos: usize = (self.len as i64 + v).try_into().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid offset")
                })?;
                self.cur = pos;
            },
            std::io::SeekFrom::Current(v) => {
                let pos: usize = (self.len as i64 + v).try_into().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid offset")
                })?;
                self.cur = pos;
            },
        }

        Ok(self.cur.try_into().unwrap())
    }
}

/// Global mutex for OutputWriter (similar to stdout)
static IS_WRITER_OPEN: Mutex<()> = Mutex::new(());

/// Construct a new handle to the current output.
///
/// Each handle returned is a reference to a shared global buffer whose access
/// is synchronized via a mutex. If you need more explicit control over
/// locking, see the [`OutputWriter::lock`] method.
#[inline(always)]
pub fn output_writer() -> OutputWriter {
    OutputWriter { _private: () }
}

/// Writer for output data (response).
pub struct OutputWriter {
    _private: (),
}

impl OutputWriter {
    /// Lock the writer to preserve access. Can result in deadlocks if improperly used.
    #[inline(always)]
    pub fn lock(&mut self) -> OutputWriterLock {
        let guard = IS_WRITER_OPEN.lock().unwrap();
        OutputWriterLock(self, guard)
    }

    /// Clear the output buffer, ie to write an error midway.
    #[inline(always)]
    pub fn clear(&mut self) {
        self.lock().clear()
    }
}

impl std::io::Write for OutputWriter {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.lock().write(buf)
    }

    #[inline(always)]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.lock().write_all(buf)
    }

    #[inline(always)]
    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        self.lock().write_fmt(fmt)
    }
}

/// Locked variant of the output writer. Prevents other writers from accessing.
#[allow(unused)]
pub struct OutputWriterLock<'a>(&'a mut OutputWriter, MutexGuard<'a, ()>);

impl OutputWriterLock<'_> {
    /// Clear the output buffer, ie to write an error midway.
    #[inline(always)]
    pub fn clear(&mut self) {
        unsafe { fn0::output_data_clear() }
    }
}

impl<'a> std::io::Write for OutputWriterLock<'a> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = unsafe { fn0::output_data_append(buf.as_ptr() as usize, buf.len()) };
        HostError::result(res)?;
        Ok(buf.len())
    }

    /// no-op
    #[inline(always)]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
