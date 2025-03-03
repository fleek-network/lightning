use anyhow::anyhow;
use fleek_service_js_poc::stream::Request;
use lightning_schema::handshake::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
use tcp_client::TcpClient;
use tokio::time::Instant;

const ADDRESS: &str = "127.0.0.1:4221";
const SERVICE_ID: u32 = 1;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (origin, uri, param) = cli::args();

    // Connect and handshake with the node
    let mut client = TcpClient::connect(ADDRESS).await?;
    client
        .send_handshake(HandshakeRequestFrame::Handshake {
            retry: None,
            service: SERVICE_ID,
            pk: [0; 96].into(),
            pop: [0; 48].into(),
        })
        .await?;

    println!("Sending request for running {uri}");

    let time = Instant::now();

    // Send the request
    client
        .send(RequestFrame::ServicePayload {
            bytes: serde_json::to_string(&Request {
                origin,
                uri,
                path: None,
                param,
                otel_headers: Default::default(),
                otel_endpoint: Default::default(),
            })
            .expect("failed to encode request")
            .into(),
        })
        .await?;

    // Read the execution response
    match client.recv().await {
        Some(ResponseFrame::ServicePayload { bytes }) => {
            let string = String::from_utf8_lossy(&bytes);
            let duration = time.elapsed();
            println!("Completed in {duration:?}:\n\n{string}");
            Ok(())
        },
        Some(ResponseFrame::Termination { reason }) => {
            Err(anyhow!("Terminated by server: {reason:?}"))
        },
        Some(frame) => Err(anyhow!("Unexpected response received: {frame:?}")),
        None => Err(anyhow!("Connection closed")),
    }
}

mod cli {
    use fleek_service_js_poc::stream::Origin;

    pub fn args() -> (Origin, String, Option<serde_json::Value>) {
        let mut args = std::env::args();
        args.next();

        let Some(uri) = args.next() else {
            help();
            std::process::exit(1);
        };

        let origin = {
            let Some(arg) = args.next() else {
                help();
                std::process::exit(1);
            };
            match arg.to_lowercase().as_str() {
                "ipfs" => Origin::Ipfs,
                "blake3" => Origin::Blake3,
                _ => {
                    help();
                    std::process::exit(1);
                },
            }
        };

        let param = args
            .next()
            .map(|v| serde_json::from_str(&v).expect("invalid json parameter"));

        (origin, uri, param)
    }

    fn help() {
        println!("Usage: ./js-poc-client <URI> <blake3|ipfs> [json entrypoint parameter]")
    }
}

mod tcp_client {
    use anyhow::Result;
    use arrayref::array_ref;
    use bytes::{Bytes, BytesMut};
    use lightning_schema::handshake::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpStream, ToSocketAddrs};

    pub struct TcpClient {
        pub stream: TcpStream,
        pub buffer: BytesMut,
    }

    impl TcpClient {
        pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
            let stream = TcpStream::connect(addr).await?;
            Ok(Self {
                stream,
                buffer: BytesMut::with_capacity(4),
            })
        }

        pub async fn recv(&mut self) -> Option<ResponseFrame> {
            loop {
                if self.buffer.len() < 4 {
                    // Read more bytes for the length delimiter
                    if 0 == self.stream.read_buf(&mut self.buffer).await.ok()? {
                        return None;
                    };
                } else {
                    // parse the length delimiter
                    let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;
                    self.buffer.reserve(len);

                    // if we need more bytes, read until we have enough
                    while self.buffer.len() < len {
                        if 0 == self.stream.read_buf(&mut self.buffer).await.ok()? {
                            return None;
                        };
                    }

                    // take the frame bytes from the buffer
                    let bytes = self.buffer.split_to(len);

                    // decode the frame
                    match ResponseFrame::decode(&bytes[4..]) {
                        Ok(frame) => return Some(frame),
                        Err(_) => {
                            eprintln!("invalid frame, dropping payload");
                            continue;
                        },
                    }
                }
            }
        }

        pub async fn send_handshake(&mut self, frame: HandshakeRequestFrame) -> Result<()> {
            self.send_inner(frame.encode()).await
        }

        pub async fn send(&mut self, frame: RequestFrame) -> Result<()> {
            self.send_inner(frame.encode()).await
        }

        async fn send_inner(&mut self, bytes: Bytes) -> Result<()> {
            self.stream.write_u32(bytes.len() as u32).await?;
            self.stream.write_all(&bytes).await?;
            Ok(())
        }
    }
}
