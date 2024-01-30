use std::fmt::Display;
use std::io::ErrorKind;

#[derive(Debug)]
pub struct Buffer {
    pub data: Vec<u8>,
    pub bytes_read: usize,
}

impl Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn hyper_error(hyper: hyper::Error) -> std::io::Error {
    std::io::Error::new(ErrorKind::Other, hyper)
}
