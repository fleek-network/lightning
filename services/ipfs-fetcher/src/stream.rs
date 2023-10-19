use anyhow::{Context, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub struct ServiceStream {
    socket: UnixStream,
    buffer: BytesMut,
}

impl ServiceStream {
    pub fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            buffer: BytesMut::with_capacity(1),
        }
    }

    pub async fn recv(&mut self) -> Option<Bytes> {
        // Read the request length delimiter
        if self.socket.read_buf(&mut self.buffer).await.ok()? == 0 {
            // Socket was closed
            return None;
        }

        // Parse and allocate for the length
        let len = self.buffer[1] as usize + 1;
        if len == 1 {
            // If the client specified it's going to send 0 bytes, this is an error
            return None;
        }
        // We reserve an additional byte for the next request
        self.buffer.reserve(len);

        // Read the request URI
        while self.buffer.len() < len {
            if self.socket.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Split and return the URI
        self.buffer.advance(1);
        Some(self.buffer.split_to(len - 1).into())
    }

    pub async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        let len = bytes.len();
        let mut buf = BytesMut::with_capacity(4 + len);
        buf.put_u32(len as u32);
        buf.put(bytes);
        self.socket
            .write_all(&buf)
            .await
            .context("failed to write outgoing bytes")
    }
}
