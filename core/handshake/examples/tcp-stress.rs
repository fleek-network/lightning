use std::fs::File;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use clap::Parser;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_handshake::schema::{HandshakeRequestFrame, ResponseFrame};
use tcp_client::*;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::Instant;

mod tcp_client {
    use anyhow::Result;
    use arrayref::array_ref;
    use bytes::{BufMut, Bytes, BytesMut};
    use lightning_handshake::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
    use serde::{Deserialize, Serialize};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tracing::warn;

    #[derive(Serialize, Deserialize)]
    pub enum Message {
        Request { chunk_len: usize, chunks: usize },
    }

    impl Message {
        pub fn encode(&self) -> Bytes {
            bincode::serialize(self)
                .expect("failed to serialize message")
                .into()
        }
    }

    pub struct TcpClient {
        pub stream: TcpStream,
        pub buffer: BytesMut,
    }

    impl TcpClient {
        pub fn new(stream: TcpStream) -> Self {
            Self {
                stream,
                buffer: BytesMut::with_capacity(4),
            }
        }

        pub async fn recv(&mut self) -> Option<ResponseFrame> {
            loop {
                if self.buffer.len() < 4 {
                    // Read more bytes for the length delimiter
                    self.stream.read_buf(&mut self.buffer).await.ok()?;
                } else {
                    // parse the length delimiter
                    let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;
                    self.buffer.reserve(len);

                    // if we need more bytes, read until we have enough
                    while self.buffer.len() < len {
                        self.stream.read_buf(&mut self.buffer).await.ok()?;
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

        async fn send_inner(&mut self, bytes: Bytes) -> Result<()> {
            self.stream.write_all(&delimit_bytes(bytes)).await?;
            Ok(())
        }

        pub async fn send_handshake(&mut self, frame: HandshakeRequestFrame) -> Result<()> {
            self.send_inner(frame.encode()).await
        }

        pub async fn request(&mut self, chunks: usize, chunk_len: usize) -> Result<()> {
            self.send_inner(
                RequestFrame::ServicePayload {
                    bytes: delimit_bytes(Message::Request { chunks, chunk_len }.encode()),
                }
                .encode(),
            )
            .await
        }
    }

    fn delimit_bytes(bytes: Bytes) -> Bytes {
        let mut buf = BytesMut::with_capacity(4 + bytes.len());
        buf.put_u32(bytes.len() as u32);
        buf.put(bytes);
        buf.into()
    }
}

/// Cli arguments
#[derive(Parser, Debug)]
struct Args {
    /// Directory to store worker data output
    #[arg(short, long, default_value = "out")]
    out: PathBuf,
    /// Address of the node to stress test
    #[arg(short, long, default_value = "127.0.0.1")]
    address: IpAddr,
    /// Port of the node to stress test
    #[arg(short, long, default_value_t = 4221)]
    port: u16,
    /// Number of concurrent workers to send requests with
    #[arg(short, long, default_value_t = 8)]
    workers: usize,
    /// Number of requests to run for each worker
    #[arg(short, long, default_value_t = 1000)]
    iters: usize,
    /// Number of chunks for each request
    #[arg(long)]
    chunks: usize,
    /// Number of bytes per chunk for each request
    #[arg(long)]
    chunk_len: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let address: SocketAddr = (args.address, args.port).into();

    if !args.out.exists() {
        std::fs::create_dir_all(&args.out).expect("failed to create out directory");
    }

    println!("spawning {} workers for {address}", args.workers);

    let mut set = JoinSet::new();
    for id in 0..args.workers {
        set.spawn(request_loop(
            args.out.clone(),
            address,
            id,
            args.workers,
            args.chunks,
            args.chunk_len,
            args.iters,
        ));
    }

    // Await all worker tasks
    while let Some(res) = set.join_next().await {
        if let Err(e) = res.expect("join error") {
            eprintln!("Error: {e}");
        }
    }

    Ok(())
}

async fn request_loop(
    out: PathBuf,
    address: SocketAddr,
    worker: usize,
    total: usize,
    chunks: usize,
    chunk_len: usize,
    iters: usize,
) -> Result<()> {
    let mut file = std::fs::File::create(out.join(format!("{address}-{worker}-{total}.json")))?;
    writeln!(file, "[")?;

    for n in 1..=iters {
        let data = run_request(address, &mut file, chunks, chunk_len).await?;

        if n != iters {
            writeln!(file, "{data},")?;
        } else {
            writeln!(file, "{data}")?;
        }
    }

    writeln!(file, "]")?;
    file.flush()?;

    Ok(())
}

async fn run_request(
    address: SocketAddr,
    file: &mut File,
    chunks: usize,
    chunk_len: usize,
) -> Result<String> {
    macro_rules! data {
        ($line:expr, $name:expr, $instant:expr) => {
            $line.push_str(&format!(
                "\"{}\": {}, ",
                $name,
                $instant.elapsed().as_nanos()
            ));
        };
    }

    let mut line = format!("\t{{\"chunks\": {chunks}, \"chunk_len\": {chunk_len}, ");
    let timer = Instant::now();

    let mut client = TcpClient::new(TcpStream::connect(address).await?);
    data!(line, "established", timer);

    // Send the handshake
    client
        .send_handshake(HandshakeRequestFrame::Handshake {
            retry: None,
            service: 1001,
            pk: ClientPublicKey([1; 96]),
            pop: ClientSignature([2; 48]),
            expiry: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3600,
            nonce: 0,
        })
        .await?;
    data!(line, "handshake_sent", timer);

    // Read hello frame from the io_stress service
    let ResponseFrame::ServicePayload { bytes } = client
        .recv()
        .await
        .ok_or(anyhow!("failed to get first byte"))?
    else {
        unreachable!()
    };
    assert_eq!(bytes.len(), 32);
    data!(line, "handshake_recv", timer);

    client.request(chunk_len, chunks).await?;
    data!(line, "request_sent", timer);

    let mut received = {
        let ResponseFrame::ServicePayload { bytes } = client
            .recv()
            .await
            .ok_or(anyhow!("failed to get first byte"))?
        else {
            unreachable!()
        };
        bytes.len()
    };
    data!(line, "first_byte_recv", timer);

    let total_bytes = chunks * chunk_len;
    while received < total_bytes {
        received += {
            let ResponseFrame::ServicePayload { bytes } = client
                .recv()
                .await
                .ok_or(anyhow!("failed to get first byte"))?
            else {
                unreachable!()
            };
            bytes.len()
        };
    }

    // Finish the json data point
    line.push_str(&format!(
        "\"last_byte_recv\": {} }}",
        timer.elapsed().as_nanos()
    ));
    file.flush()?;

    Ok(line)
}
