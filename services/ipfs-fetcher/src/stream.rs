use anyhow::Result;
use bytes::{Buf, Bytes, BytesMut};
use fn_sdk::connection::Connection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct ServiceStream {
    pub conn: Connection,
    buffer: BytesMut,
}

impl ServiceStream {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            buffer: BytesMut::with_capacity(1),
        }
    }

    pub async fn read_request(&mut self) -> Option<Bytes> {
        // Read the request length delimiter
        if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
            // Socket was closed
            return None;
        }

        // Parse and allocate for the length
        let len = self.buffer[0] as usize + 1;
        if len == 1 {
            // If the client specified it's going to send 0 bytes, this is an error
            return None;
        }
        // We reserve an additional byte for the next request
        self.buffer.reserve(len);

        // Read the request URI
        while self.buffer.len() < len {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Split and return the URI
        self.buffer.advance(1);
        Some(self.buffer.split_to(len - 1).into())
    }

    pub async fn send_payload(&mut self, bytes: &[u8]) -> Result<()> {
        self.conn.start_write(bytes.len()).await?;
        self.conn.write_all(bytes).await?;
        Ok(())
    }
}
