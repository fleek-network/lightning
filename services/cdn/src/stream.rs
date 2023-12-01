use anyhow::Result;
use arrayref::array_ref;
use bytes::{Bytes, BytesMut};
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
        // Read the payload length delimiter
        while self.buffer.len() < 4 {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Parse and allocate for the length
        let bytes = self.buffer.split_to(4);
        let len = u32::from_be_bytes(*array_ref!(bytes, 0, 4)) as usize;
        if len == 0 || len > 1024 {
            // If the client specified it's going to send 0 bytes, this is an error
            return None;
        }
        // We reserve an additional byte for the next request
        self.buffer.reserve(len + 4);

        // Read the request URI
        while self.buffer.len() < len {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Split and return the URI
        Some(self.buffer.split_to(len).into())
    }

    pub async fn send_payload(&mut self, bytes: &[u8]) -> Result<()> {
        self.conn.start_write(bytes.len()).await?;
        self.conn.write_all(bytes).await?;
        Ok(())
    }
}
