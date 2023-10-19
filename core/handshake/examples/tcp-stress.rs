use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Result;
use bytes::BytesMut;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_handshake::schema::{HandshakeRequestFrame, ResponseFrame};
use tcp_client::*;
use tokio::net::TcpStream;
use tokio::time::Instant;

const NUM_WORKERS: usize = 1;
static COUNTER: AtomicUsize = AtomicUsize::new(0);

mod tcp_client {
    use anyhow::Result;
    use arrayref::array_ref;
    use bytes::{BufMut, Bytes, BytesMut};
    use lightning_handshake::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
    use tracing::warn;

    pub struct TcpReader {
        pub reader: OwnedReadHalf,
        pub buffer: BytesMut,
    }

    impl TcpReader {
        pub async fn recv(&mut self) -> Option<ResponseFrame> {
            loop {
                if self.buffer.len() < 4 {
                    // Read more bytes for the length delimiter
                    self.reader.read_buf(&mut self.buffer).await.ok()?;
                } else {
                    // parse the length delimiter
                    let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;
                    self.buffer.reserve(len);

                    // if we need more bytes, read until we have enough
                    while self.buffer.len() < len {
                        self.reader.read_buf(&mut self.buffer).await.ok()?;
                    }

                    // take the frame bytes from the buffer
                    let bytes = self.buffer.split_to(len);

                    // decode the frame
                    match ResponseFrame::decode(&bytes[4..]) {
                        Ok(frame) => return Some(frame),
                        Err(_) => {
                            warn!("invalid frame from client, dropping payload");
                            continue;
                        },
                    }
                }
            }
        }
    }

    pub struct TcpWriter(pub OwnedWriteHalf);

    impl TcpWriter {
        async fn send_inner(&mut self, bytes: Bytes) -> Result<()> {
            let mut buf = BytesMut::with_capacity(4 + bytes.len());
            buf.put_u32(bytes.len() as u32);
            buf.put(bytes);
            self.0.write_all(&buf).await?;
            Ok(())
        }

        pub async fn send_handshake(&mut self, frame: HandshakeRequestFrame) -> Result<()> {
            self.send_inner(frame.encode()).await
        }

        pub async fn _send(&mut self, frame: RequestFrame) -> Result<()> {
            self.send_inner(frame.encode()).await
        }
    }
}

async fn handshake_only() -> Result<()> {
    let (r, w) = TcpStream::connect(("127.0.0.1", 4221)).await?.into_split();
    let mut reader = TcpReader {
        reader: r,
        buffer: BytesMut::with_capacity(4),
    };
    let mut writer = TcpWriter(w);

    // Send the handshake
    writer
        .send_handshake(HandshakeRequestFrame::Handshake {
            retry: None,
            service: 1001,
            pk: ClientPublicKey([1; 96]),
            pop: ClientSignature([2; 48]),
        })
        .await?;

    // Read hello frame from the io_stress service
    match reader.recv().await {
        Some(ResponseFrame::ServicePayload { .. }) => {
            COUNTER.fetch_add(1, Ordering::Relaxed);
        },
        Some(other) => panic!("Got other frame: {other:?}"),
        None => panic!("Failed to receive hello frame"),
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    for i in 0..NUM_WORKERS {
        tokio::spawn(async move {
            println!("Spawned worker task {i}");
            loop {
                if let Err(e) = handshake_only().await {
                    panic!("Failed to handshake: {e}");
                }
            }
        });
    }

    // Main loop displaying the number of handshakes completed in the last second.
    let mut timestamp = Instant::now();
    loop {
        timestamp += Duration::from_secs(1);
        tokio::time::sleep_until(timestamp).await;
        let count = COUNTER.swap(0, Ordering::Relaxed);
        let mut out = std::io::stdout();
        write!(out, "\r\x1b[K{} handshake/second", count).unwrap();
        out.flush().unwrap();
    }
}
