use arrayref::array_ref;
use bytes::{Buf, BufMut, BytesMut};
use lightning_handshake::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
use tcp_client::TcpClient;

const ADDRESS: &str = "127.0.0.1:4221";
const CID: &str = "bafybeibi5vlbuz3jstustlxbxk7tmxsyjjrxak6us4yqq6z2df3jwidiwi";
const SERVICE_ID: u32 = 0;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // Send a request for the CID.
    let mut buf = BytesMut::with_capacity(1 + CID.len());
    buf.put_u8(CID.len() as u8);
    buf.put(CID.as_bytes());
    client
        .send(RequestFrame::ServicePayload { bytes: buf.into() })
        .await?;

    let mut buffer = BytesMut::new();
    let mut total = 0;

    // Read the number of blocks we should receive back.
    let Some(ResponseFrame::ServicePayload { bytes }) = client.recv().await else {
        panic!("invalid or no response received");
    };
    if bytes.len() > 4 {
        buffer.put(&bytes[4..]);
    }
    let num_blocks = u32::from_be_bytes(*array_ref![bytes, 0, 4]);

    // Stream the remaining content
    for _ in 0..num_blocks {
        // Read payloads until we have the length of this block
        while buffer.len() < 4 {
            let Some(ResponseFrame::ServicePayload { bytes }) = client.recv().await else {
                panic!("invalid or no response received");
            };
            buffer.put(bytes);
        }

        // Read payloads until we have the entire block
        let len = u32::from_be_bytes(*array_ref![buffer, 0, 4]) as usize + 4;
        while buffer.len() < len {
            let Some(ResponseFrame::ServicePayload { bytes }) = client.recv().await else {
                panic!("invalid or no response received");
            };
            buffer.put(bytes)
        }

        // Drop the block from memory (we don't need them for this example)
        buffer.advance(len);
        total += len;
    }

    println!("Successfully streamed {total} bytes for {CID}");

    Ok(())
}

mod tcp_client {
    use anyhow::Result;
    use arrayref::array_ref;
    use bytes::{BufMut, Bytes, BytesMut};
    use lightning_handshake::schema::{HandshakeRequestFrame, RequestFrame, ResponseFrame};
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
            let mut buf = BytesMut::with_capacity(4 + bytes.len());
            buf.put_u32(bytes.len() as u32);
            buf.put(bytes);
            self.stream.write_all(&buf).await?;
            Ok(())
        }
    }
}
