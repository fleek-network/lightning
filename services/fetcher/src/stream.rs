use anyhow::Result;
use arrayref::array_ref;
use bytes::{Buf, Bytes, BytesMut};
use fn_sdk::connection::Connection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::Origin;

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

    pub async fn read_request(&mut self) -> Option<(Origin, Bytes)> {
        // Read the payload length delimiter
        while self.buffer.len() < 5 {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Parse and allocate for the length
        let bytes = self.buffer.split_to(4);
        let len = u32::from_be_bytes(*array_ref!(bytes, 0, 4)) as usize;
        if len == 0 || len > 1025 {
            // If the client specified it's going to send 0 bytes, this is an error
            return None;
        }

        // We reserve an additional byte for the next request
        self.buffer.reserve(len + 4);

        while self.buffer.is_empty() {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Read the origin type
        let origin = Origin::from(self.buffer[0]);

        // Read the request URI
        while self.buffer.len() < len {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Split and return the URI
        self.buffer.advance(1);
        Some((origin, self.buffer.split_to(len - 1).into()))
    }

    pub async fn send_payload(&mut self, bytes: &[u8]) -> Result<()> {
        self.conn.start_write(bytes.len()).await?;
        self.conn.write_all(bytes).await?;
        Ok(())
    }
}
