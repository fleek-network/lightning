use anyhow::{Context, Result};
use arrayref::array_ref;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Serialize, Deserialize)]
pub enum Message {
    Request { chunk_len: usize, chunks: usize },
}

impl Message {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).context("failed to decode message")
    }

    pub fn encode(&self) -> Bytes {
        bincode::serialize(self)
            .expect("failed to serialize message")
            .into()
    }
}

struct ServiceStream {
    socket: UnixStream,
    buffer: BytesMut,
}

impl ServiceStream {
    fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            buffer: BytesMut::with_capacity(4),
        }
    }

    async fn recv(&mut self) -> Option<Message> {
        loop {
            if self.buffer.len() < 4 {
                // Read more bytes for the length delimiter
                if self.socket.read_buf(&mut self.buffer).await.ok()? == 0 {
                    return None;
                };
            } else {
                // Parse the length delimiter
                let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;
                // TODO: Don't re-allocate here if the future is canceled.
                self.buffer.reserve(len);

                // If we need more bytes, read until we have enough
                while self.buffer.len() < len {
                    if self.socket.read_buf(&mut self.buffer).await.ok()? == 0 {
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

    async fn send(&mut self, size: usize) -> Result<()> {
        // Write the buffer to the socket
        self.socket
            .write_all(&vec![17; size])
            .await
            .context("failed to write frame to socket")
    }
}

async fn connection_loop(socket: UnixStream) {
    let mut stream = ServiceStream::new(socket);
    stream.send(32).await.expect("failed to send hello message");

    while let Some(Message::Request { chunk_len, chunks }) = stream.recv().await {
        // send n chunks with a certain length
        for _ in 0..chunks {
            stream
                .send(chunk_len)
                .await
                .expect("failed to send chunk message");
        }
    }
}

pub async fn main() {
    fn_sdk::ipc::init_from_env();
    println!("Running io_stress service!");

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok((socket, _)) = listener.accept().await {
        tokio::spawn(connection_loop(socket));
    }
}
