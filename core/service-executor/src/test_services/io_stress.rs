use anyhow::{Context, Result};
use arrayref::array_ref;
use bytes::{Bytes, BytesMut};
use fn_sdk::connection::Connection;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::info;

#[derive(Serialize, Deserialize)]
pub enum Message {
    Request { chunk_len: usize, chunks: usize },
}

impl Message {
    #[inline(always)]
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).context("failed to decode message")
    }

    #[inline(always)]
    pub fn encode(&self) -> Bytes {
        bincode::serialize(self)
            .expect("failed to serialize message")
            .into()
    }
}

struct ServiceStream {
    conn: Connection,
    buffer: BytesMut,
}

impl ServiceStream {
    #[inline(always)]
    fn new(conn: Connection) -> Self {
        Self {
            conn,
            buffer: BytesMut::with_capacity(4),
        }
    }

    #[inline(always)]
    async fn read_message(&mut self) -> Option<Message> {
        loop {
            if self.buffer.len() < 4 {
                // Read more bytes for the length delimiter
                if self.conn.read_buf(&mut self.buffer).await.ok()? == 0 {
                    return None;
                };
            } else {
                // Parse the length delimiter
                let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;
                // TODO: Don't re-allocate here if the future is canceled.
                self.buffer.reserve(len);

                // If we need more bytes, read until we have enough
                while self.buffer.len() < len {
                    if self.conn.read_buf(&mut self.buffer).await.ok()? == 0 {
                        return None;
                    };
                }

                // Take the frame bytes from the buffer
                let bytes = self.buffer.split_to(len);
                // Decode the frame
                match Message::decode(&bytes[4..]) {
                    Ok(frame) => return Some(frame),
                    Err(e) => {
                        eprintln!("{e}");
                        continue;
                    },
                }
            }
        }
    }

    #[inline(always)]
    async fn send_payload(&mut self, size: usize) -> Result<()> {
        // Write the buffer to the socket
        self.conn.start_write(size).await?;
        self.conn.write_all(&vec![17; size]).await?;
        Ok(())
    }
}

#[inline(always)]
async fn connection_loop(conn: Connection) {
    let mut stream = ServiceStream::new(conn);
    stream
        .send_payload(32)
        .await
        .expect("failed to send hello message");

    while let Some(Message::Request { chunk_len, chunks }) = stream.read_message().await {
        // send n chunks with a certain length
        for _ in 0..chunks {
            stream
                .send_payload(chunk_len)
                .await
                .expect("failed to send chunk message");
        }
    }
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    info!("Running io_stress service!");

    let mut listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(connection_loop(conn));
    }
}
