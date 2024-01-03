use std::net::SocketAddr;

use anyhow::Result;
use arrayref::array_ref;
use async_trait::async_trait;
use axum::Router;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{delimit_frame, Transport, TransportReceiver, TransportSender};
use crate::schema::{self, RES_SERVICE_PAYLOAD_TAG};
use crate::shutdown::ShutdownWaiter;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TcpConfig {
    pub address: SocketAddr,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4221).into(),
        }
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
        let listener = TcpListener::bind(config.address).await?;
        info!("Binding TCP transport to {}", config.address);
        // bounded channel to provide some back pressure for incoming connections
        let (tx, rx) = mpsc::channel(256);

        // Spawn the main loop accepting connections until shutdown
        tokio::spawn(async move {
            loop {
                // Accept a new stream from the listener
                tokio::select! {
                    res = listener.accept() => {
                        match res {
                            Ok((stream, _)) => spawn_handshake_task(stream, tx.clone()),
                            _ => break,
                        }
                    },
                    _ = shutdown.wait_for_shutdown() => break,
                }
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

fn spawn_handshake_task(
    mut stream: TcpStream,
    tx: mpsc::Sender<(schema::HandshakeRequestFrame, TcpSender, TcpReceiver)>,
) {
    tokio::spawn(async move {
        let mut buf = BytesMut::with_capacity(4);

        // Read until we have enough for the length delimiter
        while buf.len() < 4 {
            match stream.read_buf(&mut buf).await {
                Ok(len) if len == 0 => return,
                Err(_) => return,
                Ok(_) => {},
            }
        }

        // Parse the length delimiter
        // TODO: Do better, there are only 3 different handshake request variants/sizes
        let len = u32::from_be_bytes(*array_ref!(buf, 0, 4)) as usize;
        if len > 157 {
            warn!("dropping connection, handshake request delimiter is too large");
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
        tx.send((frame, TcpSender::spawn(writer), TcpReceiver::new(reader)))
            .await
            .ok();
    });
}

/// Driver loop to write outgoing bytes directly to the stream.
async fn spawn_write_driver(mut writer: OwnedWriteHalf, rx: async_channel::Receiver<Bytes>) {
    while let Ok(bytes) = rx.recv().await {
        if let Err(e) = writer.write_all(&bytes).await {
            warn!("Dropping payload, failed to write to stream: {e}");
        };
    }
}

pub struct TcpSender {
    sender: async_channel::Sender<Bytes>,
    current_write: u32,
}

impl TcpSender {
    /// Create the [`TcpSender`], additionally spawning a task to handle writing bytes to the
    /// stream.
    #[inline(always)]
    pub fn spawn(writer: OwnedWriteHalf) -> Self {
        let (sender, receiver) = async_channel::unbounded();
        tokio::spawn(spawn_write_driver(writer, receiver));

        Self {
            sender,
            current_write: 0,
        }
    }

    #[inline(always)]
    fn send_inner(&mut self, bytes: Bytes) {
        if let Err(e) = self.sender.try_send(bytes) {
            warn!("payload dropped, failed to send to write loop: {e}");
        }
    }
}

impl TransportSender for TcpSender {
    fn send_handshake_response(&mut self, response: schema::HandshakeResponse) {
        self.send_inner(delimit_frame(response.encode()));
    }

    fn send(&mut self, frame: schema::ResponseFrame) {
        debug_assert!(
            !matches!(
                frame,
                schema::ResponseFrame::ServicePayload { .. }
                    | schema::ResponseFrame::ServicePayloadChunk { .. }
            ),
            "payloads should only be sent via start_write and write"
        );

        let bytes = delimit_frame(frame.encode());
        self.send_inner(bytes);
    }

    fn start_write(&mut self, len: usize) {
        let len = len as u32;
        debug_assert!(
            self.current_write == 0,
            "data should be written completely before calling start_write again"
        );

        self.current_write = len;

        let mut buffer = Vec::with_capacity(5);
        // add 1 to the delimiter to include the frame tag
        buffer.put_u32(len + 1);
        buffer.put_u8(RES_SERVICE_PAYLOAD_TAG);
        // write the delimiter and payload tag to the stream
        self.send_inner(buffer.into());
    }

    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        let len = buf.len() as u32;
        debug_assert!(self.current_write != 0);
        debug_assert!(self.current_write >= len);

        self.current_write -= len;
        self.send_inner(buf.to_vec().into());
        Ok(buf.len())
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
            buffer: BytesMut::with_capacity(4),
        }
    }
}

#[async_trait]
impl TransportReceiver for TcpReceiver {
    /// Cancel Safety:
    /// This method is cancel safe, but could potentially allocate multiple times for the delimiter
    /// if canceled.
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        loop {
            if self.buffer.len() < 4 {
                // Read more bytes for the length delimiter
                if self.reader.read_buf(&mut self.buffer).await.ok()? == 0 {
                    return None;
                };
            } else {
                // Parse the length delimiter
                let len = u32::from_be_bytes(*array_ref!(self.buffer, 0, 4)) as usize + 4;
                // TODO: Don't re-allocate here if the future is canceled.
                self.buffer.reserve(len);

                // If we need more bytes, read until we have enough
                while self.buffer.len() < len {
                    if self.reader.read_buf(&mut self.buffer).await.ok()? == 0 {
                        return None;
                    };
                }

                // Take the frame bytes from the buffer
                let bytes = self.buffer.split_to(len);
                // Decode the frame
                match schema::RequestFrame::decode(&bytes[4..]) {
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
        let config = TcpConfig {
            address: ([127, 0, 0, 1], 20000).into(),
        };
        let (mut transport, _) = TcpTransport::bind(notifier.waiter(), config.clone()).await?;

        // Connect a dummy client
        let mut client = TcpStream::connect(config.address)
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
