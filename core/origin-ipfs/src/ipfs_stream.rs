use std::io::{self, ErrorKind};
use std::pin::Pin;
use std::task::Poll;

use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use futures::ready;
use hyper::Body;
use lightning_interfaces::UntrustedStream;

#[derive(Debug)]
pub struct IPFSStream {
    body: Body,
    requested_cid: Cid,
    data: Vec<u8>,
    done: bool,
}

impl IPFSStream {
    pub fn new(requested_cid: Cid, body: Body) -> Self {
        Self {
            body,
            requested_cid,
            data: Vec::new(),
            done: false,
        }
    }
}

impl tokio_stream::Stream for IPFSStream {
    type Item = Result<bytes::Bytes, io::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let body = Pin::new(&mut self.body);

        match ready!(body.poll_next(cx)) {
            Some(Ok(bytes)) => {
                self.data.extend(bytes.as_ref());
                Poll::Ready(Some(Ok(bytes)))
            },
            Some(Err(err)) => {
                Poll::Ready(Some(Err(io::Error::new(ErrorKind::Other, Box::new(err)))))
            },
            None => {
                self.done = true;
                Poll::Ready(None)
            },
        }
    }
}

impl UntrustedStream for IPFSStream {
    fn was_content_valid(&self) -> Option<bool> {
        if !self.done {
            return None;
        }
        match Code::try_from(self.requested_cid.hash().code()) {
            Ok(hasher) => Some(&hasher.digest(&self.data) == self.requested_cid.hash()),
            _ => Some(false),
        }
    }
}
