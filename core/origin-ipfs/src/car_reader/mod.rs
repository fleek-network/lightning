use std::io::ErrorKind;

use anyhow::Result;
use cid::Cid;
use tokio::io::AsyncRead;

pub struct CarReader<R: AsyncRead + Unpin> {
    reader: R,
}

impl<R: AsyncRead + Unpin> CarReader<R> {
    pub fn new(reader: R) -> Self {
        todo!()
    }

    pub async fn next_block(&mut self) -> Result<Option<(Cid, Vec<u8>)>> {
        todo!()
    }
}

pub fn hyper_error(hyper: hyper::Error) -> std::io::Error {
    std::io::Error::new(ErrorKind::Other, hyper)
}
