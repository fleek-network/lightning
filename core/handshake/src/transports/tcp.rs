use anyhow::Result;
use arrayref::array_ref;
use async_trait::async_trait;
use axum::Router;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use triomphe::Arc;

use super::{Transport, TransportReceiver, TransportSender};
use crate::schema;
use crate::shutdown::ShutdownWaiter;

#[derive(Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    port: u16,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self { port: 4221 }
    }
}

pub struct TcpTransport {
    rx: mpsc::Receiver<(schema::HandshakeRequestFrame, TcpSender, TcpReceiver)>,
}

#[async_trait]
impl Transport for TcpTransport {
    type Config = TcpConfig;
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    async fn bind(
        shutdown: ShutdownWaiter,
        config: Self::Config,
    ) -> Result<(Self, Option<Router>)> {
        let listener = TcpListener::bind(("0.0.0.0", config.port)).await?;
        info!("Binding TCP transport to 0.0.0.0:{}", config.port);
        // bounded channel to provide some back pressure for incoming connections
        let (tx, rx) = mpsc::channel(256);

        // Spawn the main loop accepting connections until shutdown
        tokio::spawn(async move {
            while !shutdown.is_shutdown() {
                // Accept a new stream from the listener
                let Ok((mut stream, _)) = listener.accept().await else { break };

                // Spawn a task for driving the connection until we've received a handshake request
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buf = BytesMut::with_capacity(157 + 4);

                    // Read until we have enough for the length delimiter
                    while buf.len() < 4 {
                        match stream.read_buf(&mut buf).await {
                            Ok(len) if len == 0 => return,
                            Err(_) => return,
                            Ok(_) => {},
                        }
                    }

                    // Parse the length delimiter
                    let len = u32::from_be_bytes(*array_ref!(buf, 0, 4)) as usize;
                    if len > 157 {
                        return;
                    }
                    buf.reserve(len);
                    buf.advance(4);

                    // Read until we have enough for the handshake frame
                    while buf.len() < len {
                        match stream.read_buf(&mut buf).await {
                            Ok(len) if len == 0 => return,
                            Err(_) => return,
                            Ok(_) => {},
                        }
                    }

                    // Parse the handshake frame
                    let Ok(frame) = schema::HandshakeRequestFrame::decode(&buf) else {
                        return
                    };

                    let (reader, writer) = stream.into_split();

                    // Send the frame and the new connection over the channel
                    tx.send((frame, TcpSender(Arc::new(writer)), TcpReceiver::new(reader)))
                        .await
                        .ok();
                });
            }
        });

        Ok((Self { rx }, None))
    }

    #[inline(always)]
    async fn accept(
        &mut self,
    ) -> Option<(schema::HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        self.rx.recv().await
    }
}

pub struct TcpSender(Arc<OwnedWriteHalf>);

impl TcpSender {
    #[inline(always)]
    fn send_inner(&mut self, bytes: Bytes) {
        let writer = self.0.clone();
        tokio::spawn(async move {
            if let Err(e) = writer.writable().await {
                warn!("failed to get writable: {e}");
            }
            let mut written = 0;
            while written < bytes.len() {
                match writer.try_write(&bytes[written..]) {
                    Ok(len) => written += len,
                    Err(e) => warn!("failed to write bytes to stream: {e}"),
                }
            }
        });
    }
}

impl TransportSender for TcpSender {
    fn send_handshake_response(&mut self, response: schema::HandshakeResponse) {
        let bytes = delimit_frame(response.encode());
        self.send_inner(bytes);
    }

    /// Should not send a payload > u32::MAX - 1
    fn send(&mut self, frame: schema::ResponseFrame) {
        let bytes = frame.encode();
        assert!(bytes.len() < u32::MAX as usize);
        self.send_inner(delimit_frame(bytes));
    }
}

pub struct TcpReceiver {
    reader: OwnedReadHalf,
    buffer: BytesMut,
}

impl TcpReceiver {
    pub fn new(reader: OwnedReadHalf) -> Self {
        Self {
            reader,
            buffer: BytesMut::with_capacity(257 << 10),
        }
    }
}

#[async_trait]
impl TransportReceiver for TcpReceiver {
    /// Async Safety: This method is cancel safe
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        loop {
            if self.buffer.len() >= 4 {
                // parse the length delimiter
                let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;

                // if we need more bytes, read until we have enough
                while self.buffer.len() < len {
                    self.reader.read_buf(&mut self.buffer).await.ok()?;
                }

                // take the frame bytes from the buffer, and reserve for the next delimiter
                self.buffer.advance(4);
                let bytes = self.buffer.split_to(len - 4);
                self.buffer.reserve(len);

                // decode the frame
                match schema::RequestFrame::decode(&bytes) {
                    Ok(frame) => return Some(frame),
                    Err(_) => {
                        warn!("invalid frame from client, dropping payload");
                        continue;
                    },
                }
            } else {
                // Read more bytes for the length delimiter
                self.reader.read_buf(&mut self.buffer).await.ok()?;
            }
        }
    }
}

pub fn delimit_frame(bytes: Bytes) -> Bytes {
    let mut buf = BytesMut::with_capacity(4 + bytes.len());
    buf.put_u32(bytes.len() as u32);
    buf.put(bytes);
    buf.into()
}

#[cfg(test)]
mod tests {
    use fleek_crypto::{ClientPublicKey, ClientSignature, NodePublicKey, NodeSignature};
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    use super::*;
    use crate::schema::{HandshakeRequestFrame, HandshakeResponse};
    use crate::shutdown::ShutdownNotifier;

    #[tokio::test(flavor = "multi_thread")]
    async fn handshake() -> Result<()> {
        // Bind the server
        let notifier = ShutdownNotifier::default();
        let config = TcpConfig::default();
        let (mut transport, _) = TcpTransport::bind(notifier.waiter(), config.clone()).await?;

        // Connect a dummy client
        let mut client = TcpStream::connect(("127.0.0.1", config.port))
            .await
            .expect("should connect");

        const REQ_FRAME: HandshakeRequestFrame = HandshakeRequestFrame::Handshake {
            retry: None,
            service: 0,
            pk: ClientPublicKey([1; 96]),
            pop: ClientSignature([2; 48]),
        };

        const RES_FRAME: HandshakeResponse = HandshakeResponse {
            pk: NodePublicKey([3; 32]),
            pop: NodeSignature([4; 64]),
        };

        // Write the handshake frame
        let bytes = delimit_frame(REQ_FRAME.encode());
        client.write_all(&bytes).await?;

        {
            // Accept the connection from the transport, which should read the handshake request
            // frame
            let (frame, mut sender, _) = transport
                .accept()
                .await
                .expect("failed to receive connection");

            assert_eq!(REQ_FRAME, frame, "received incorrect request frame");

            // Send the response frame
            sender.send_handshake_response(RES_FRAME);

            // Drop the connection
        }

        // Read the response frame (server drops the connection after, so read_to_end works)
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await?;
        let frame = HandshakeResponse::decode(&buf[4..])?;
        assert_eq!(
            frame, RES_FRAME,
            "received incorrect handshake response frame"
        );

        Ok(())
    }
}
