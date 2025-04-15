use std::borrow::BorrowMut;
use std::future::Future;
use std::io::{Error as IoError, Result as IoResult};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use b3fs::bucket::Bucket;
use b3fs::collections::async_hashtree::AsyncHashTree;
use b3fs::stream::ProofBuf;
use bytes::{BufMut, BytesMut};
use futures::{ready, FutureExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;

type Block = (Vec<u8>, ProofBuf);

trait ReadFut: Future<Output = Result<Block, std::io::Error>> + Send + Sync {}
impl<T: Future<Output = Result<Block, std::io::Error>> + Send + Sync> ReadFut for T {}

/// Leading bit for flagging proofs and chunks.
/// Payloads are either a proof segment (max ~4MiB initial proof),
/// or a chunk (256KiB), so this bit should be safe to use.
const LEADING_BIT: u32 = 1 << 31;

/// Owned blockstore request stream. Responsible for writing a verified
/// stream of blockstore content to `AsyncRead` calls. Drops all writes.
pub struct VerifiedStream {
    current: u32,
    bucket: Bucket,
    tree: Arc<Mutex<AsyncHashTree<tokio::fs::File>>>,
    num_blocks: u32,
    read_fut: Option<Pin<Box<dyn ReadFut>>>,
    buffer: BytesMut,
    data_sent: usize,
}

impl VerifiedStream {
    pub async fn new(hash: &[u8; 32], blockstore_path: &Path) -> Result<Self, IoError> {
        if !fn_sdk::api::fetch_blake3(*hash).await {
            return Err(IoError::new(
                std::io::ErrorKind::NotFound,
                "failed to fetch blake3 hash",
            ));
        }

        let bucket = Bucket::open(blockstore_path).await?;
        let content_header = bucket.get(hash).await?;
        let num_blocks = content_header.blocks();
        let Some(file) = content_header.into_file() else {
            return Err(IoError::new(
                std::io::ErrorKind::Other,
                "only files are supported right now",
            ));
        };

        let tree = file
            .hashtree()
            .await
            .map_err(|e| IoError::new(std::io::ErrorKind::Other, e))?;

        let tree = Arc::new(Mutex::new(tree));

        // TODO: Estimate and validate content length limits, based on the
        //       number of chunk hashes in the proof.

        // Create the buffer and write the starting proof to it right away
        let buffer = BytesMut::new();

        let current = 0;

        let tree_clone = tree.clone();
        let bucket_clone = bucket.clone();
        let read_fut = Some(Box::pin(async move {
            let mut tree = tree_clone.lock().await;

            let hash = tree
                .get_hash(current)
                .await
                .map_err(|e| IoError::new(std::io::ErrorKind::Other, e))?;

            let proof = tree
                .generate_proof(current)
                .await
                .map_err(|e| IoError::new(std::io::ErrorKind::Other, e))?;

            let Some(hash) = hash else {
                return Err(IoError::new(std::io::ErrorKind::Other, "hash not found"));
            };

            let data = bucket_clone.get_block_content(&hash).await?;

            let Some(data) = data else {
                return Err(IoError::new(std::io::ErrorKind::Other, "data not found"));
            };

            Ok((data, proof))
        }) as _);

        Ok(Self {
            current,
            bucket,
            tree,
            num_blocks,
            read_fut,
            buffer,
            data_sent: 0,
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
            if !self.buffer.is_empty() {
                let len = buf.remaining().min(self.buffer.len());
                let bytes = self.buffer.split_to(len);
                buf.put_slice(&bytes);

                return Poll::Ready(Ok(()));
            }

            // poll pending read call
            if let Some(fut) = self.read_fut.borrow_mut() {
                match ready!(fut.poll_unpin(cx)) {
                    Ok((block, proof)) => {
                        // remove future
                        self.read_fut = None;

                        // write proof
                        // TODO(matthias): check if empty?
                        // if !proof.is_empty() {
                        self.buffer.put_u32(proof.len() as u32);
                        self.buffer.put_slice(proof.as_slice());
                        // }

                        // write chunk payload (with leading bit set)
                        self.buffer.put_u32(block.len() as u32 | LEADING_BIT);
                        self.buffer.put_slice(&block);
                        self.data_sent += block.len();

                        // flush read buffer
                        continue;
                    },
                    Err(e) => return Poll::Ready(Err(e)),
                }
            }

            // queue the next block read
            self.current += 1;
            let current = self.current;

            // exit if we finished the last block
            if self.num_blocks == self.current {
                // This tells the receiver that the stream ended
                self.buffer.put_u32(LEADING_BIT);
                continue;
            }

            let bucket_clone = self.bucket.clone();
            let tree_clone = self.tree.clone();
            self.read_fut = Some(Box::pin(async move {
                let mut tree = tree_clone.lock().await;

                let hash = tree
                    .get_hash(current)
                    .await
                    .map_err(|e| IoError::new(std::io::ErrorKind::Other, e))?;

                let proof = tree
                    .generate_proof(current)
                    .await
                    .map_err(|e| IoError::new(std::io::ErrorKind::Other, e))?;

                let Some(hash) = hash else {
                    return Err(IoError::new(std::io::ErrorKind::Other, "hash not found"));
                };

                let data = bucket_clone.get_block_content(&hash).await?;

                let Some(data) = data else {
                    return Err(IoError::new(std::io::ErrorKind::Other, "data not found"));
                };

                Ok((data, proof))
            }) as _);

            continue;
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
