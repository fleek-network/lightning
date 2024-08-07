use std::borrow::BorrowMut;
use std::future::Future;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use blake3_tree::ProofBuf;
use bytes::{BufMut, BytesMut};
use fn_sdk::blockstore::ContentHandle;
use futures::{ready, FutureExt};
use tokio::io::{AsyncRead, AsyncWrite};

trait ReadFut: Future<Output = Result<Vec<u8>, std::io::Error>> + Send + Sync {}
impl<T: Future<Output = Result<Vec<u8>, std::io::Error>> + Send + Sync> ReadFut for T {}

/// Owned blockstore request stream. Responsible for writing a verified
/// stream of blockstore content to `AsyncRead` calls. Drops all writes.
pub struct VerifiedStream {
    current: usize,
    handle: Arc<ContentHandle>,
    read_fut: Option<Pin<Box<dyn ReadFut>>>,
    read_buf: BytesMut,
}

impl VerifiedStream {
    pub async fn new(hash: &[u8; 32]) -> Result<Self, std::io::Error> {
        fn_sdk::blockstore::ContentHandle::load(hash)
            .await
            .map(|handle| {
                let mut read_buf = BytesMut::new();

                // write starting proof to the buffer
                let proof = ProofBuf::new(handle.tree.as_ref(), 0);
                read_buf.put_u8(0);
                read_buf.put_u32(proof.len() as u32);
                read_buf.put_slice(proof.as_slice());

                Self {
                    current: 0,
                    handle: handle.into(),
                    read_fut: None,
                    read_buf,
                }
            })
    }
}

impl AsyncRead for VerifiedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut tokio::io::ReadBuf,
    ) -> Poll<IoResult<()>> {
        loop {
            // flush as many bytes as possible
            if !self.read_buf.is_empty() {
                let len = buf.remaining().min(self.read_buf.len());
                let bytes = self.read_buf.split_to(len);
                buf.put_slice(&bytes);

                return Poll::Ready(Ok(()));
            }

            // poll pending block
            if let Some(fut) = self.read_fut.borrow_mut() {
                match ready!(fut.poll_unpin(cx)) {
                    Ok(block) => {
                        // Write block payload
                        self.read_buf.put_u32(block.len() as u32);
                        self.read_buf.put_u8(1);
                        self.read_buf.put_slice(&block);

                        // remove future and loop to flush read buf
                        self.read_fut = None;
                        continue;
                    },
                    Err(e) => return Poll::Ready(Err(e)),
                }
            }

            // exit if we finished the last block
            if self.handle.len() == self.current {
                // Since no data was written, this is effectively an EOF signal.
                return Poll::Ready(Ok(()));
            }

            // queue the next block read
            self.current += 1;
            let current = self.current;
            let handle = self.handle.clone();
            let fut = Box::pin(async move { handle.read(current).await });
            self.read_fut = Some(fut);

            // write next proof payload
            let proof = ProofBuf::resume(self.handle.tree.as_ref(), self.current);
            self.read_buf.put_u32(proof.len() as u32 + 1);
            self.read_buf.put_u8(0);
            self.read_buf.put_slice(proof.as_slice());
        }
    }
}

// ignore all input
impl AsyncWrite for VerifiedStream {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context, buf: &[u8]) -> Poll<IoResult<usize>> {
        Poll::Ready(IoResult::Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<IoResult<()>> {
        Poll::Ready(IoResult::Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<IoResult<()>> {
        Poll::Ready(IoResult::Ok(()))
    }
}
