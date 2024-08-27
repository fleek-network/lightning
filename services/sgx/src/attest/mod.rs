use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use arrayref::array_ref;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{ready, FutureExt};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// TODO: replace with something real
#[derive(Debug)]
pub struct EndpointState {}

impl EndpointState {
    /// Handle an incoming request from the enclave
    async fn handle(self: Arc<Self>, request: Request) -> std::io::Result<Bytes> {
        match request {
            Request::TargetInfo => {
                todo!("fetch target info")
            },
            Request::Quote(_report) => {
                todo!("get quote")
            },
            Request::Collateral(_quote) => {
                todo!("get collateral")
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
enum Request {
    TargetInfo,
    Quote(Vec<u8>),
    Collateral(Vec<u8>),
}

pub struct AttestationEndpoint {
    method: Box<str>,
    // input buffer
    buffer: BytesMut,
    // response future
    fut: Option<Pin<Box<dyn HandleFut>>>,
    // output buffer
    output: Option<Bytes>,
    // quote provider stuff
    state: Arc<EndpointState>,
}

trait HandleFut: Future<Output = Result<Bytes, std::io::Error>> + Send + Sync {}
impl<T: Future<Output = Result<Bytes, std::io::Error>> + Send + Sync> HandleFut for T {}

impl AttestationEndpoint {
    pub fn new(method: &str, provider: Arc<EndpointState>) -> Self {
        Self {
            method: method.into(),
            state: provider,
            buffer: BytesMut::new(),
            fut: None,
            output: None,
        }
    }
}

impl AsyncWrite for AttestationEndpoint {
    /// Write bytes into the buffer, and attempt to use them,
    /// returning for more bytes if needed.
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.method.as_ref() != "target_info" {
            // push bytes into buffer
            self.buffer.put_slice(buf);

            // return for more bytes if needed
            if self.buffer.len() < 4 {
                return std::task::Poll::Ready(Ok(buf.len()));
            }

            // read len delim
            let len = u32::from_be_bytes(*array_ref![self.buffer, 0, 4]) as usize;

            // return for more bytes if needed
            if self.buffer.len() < len + 4 {
                return std::task::Poll::Ready(Ok(buf.len()));
            }

            // parse request payload
            self.buffer.advance(4);

            // set handler future to be polled from `AsyncRead`
            let req = match self.method.as_ref() {
                "quote" => Request::Quote(self.buffer.split().to_vec()),
                "collateral" => Request::Collateral(self.buffer.split().to_vec()),
                _ => unreachable!(),
            };
            self.fut = Some(Box::pin(self.state.clone().handle(req)));
        }
        std::task::Poll::Ready(Ok(buf.len()))
    }

    /// no-op
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncRead for AttestationEndpoint {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> std::task::Poll<std::io::Result<()>> {
        // flush all output
        if let Some(output) = self.output.as_mut() {
            let len = buf.remaining().min(output.len());
            buf.put_slice(&output.split_to(len));
            return std::task::Poll::Ready(Ok(()));
        }

        // poll handler future if active
        match &mut self.fut {
            Some(fut) => match ready!(fut.poll_unpin(cx)) {
                Ok(mut output) => {
                    // write as much as possible, storing the rest for later
                    let len = buf.remaining().min(output.len());
                    buf.put_slice(&output.split_to(len));
                    self.output = Some(output);
                    std::task::Poll::Ready(Ok(()))
                },
                Err(e) => std::task::Poll::Ready(Err(e)),
            },
            None => std::task::Poll::Pending,
        }
    }
}
