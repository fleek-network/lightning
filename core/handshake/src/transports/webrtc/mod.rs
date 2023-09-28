mod signal;
mod worker;

use std::net::SocketAddr;
use std::sync::Arc;

use affair::{Executor, Socket, TokioSpawn};
use async_trait::async_trait;
use axum::Router;
use bytes::Bytes;
use log::error;
use serde::{Deserialize, Serialize};
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;

use self::worker::{IncomingConnectionWorker, SendWorker};
use super::{Transport, TransportReceiver, TransportSender};
use crate::schema::{self, HandshakeRequestFrame, RequestFrame};
use crate::shutdown::ShutdownWaiter;
use crate::transports::webrtc::signal::router;

/// Maximum size for an outgoing payload.
const MAX_PAYLOAD_SIZE: usize = 32 << 10;

#[derive(Serialize, Deserialize, Clone)]
pub struct WebRtcConfig {
    /// Address to listen on for the signaling server. This is used to receive and respond to
    /// incoming RTC Session Descriptions, to negotiate a new SRTP connection.
    pub signal_address: SocketAddr,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            signal_address: ([0, 0, 0, 0], 4210).into(),
        }
    }
}

/// A WebRTC Transport. Spawns a HTTP signaling server, and binds to ephemeral UDP ports per
/// peer connection.
pub struct WebRtcTransport {
    /// Receiver for incoming DataChannels from peer connections.
    conn_rx: tokio::sync::mpsc::Receiver<(HandshakeRequestFrame, Arc<RTCDataChannel>)>,
}

#[async_trait]
impl Transport for WebRtcTransport {
    type Config = WebRtcConfig;
    type Sender = WebRtcSender;
    type Receiver = WebRtcReceiver;

    async fn bind(
        _waiter: ShutdownWaiter, // We use the signaling endpoint to dictate new connections, so we
        // don't need the waiter directly here.
        config: Self::Config,
    ) -> anyhow::Result<(Self, Option<Router>)> {
        log::info!("Binding WebRTC transport on {}", config.signal_address);

        let (conn_tx, conn_rx) = tokio::sync::mpsc::channel(16);

        // Spawn a worker for handling new connection setup.
        let worker = IncomingConnectionWorker { conn_tx };
        let socket = TokioSpawn::spawn_async(worker);

        // Add our sdp endpoint to the http router.
        let router = router(socket);

        Ok((Self { conn_rx }, Some(router)))
    }

    async fn accept(
        &mut self,
    ) -> Option<(schema::HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        let (req, data_channel) = self.conn_rx.recv().await?;

        // Setup message receiver channel and callback method.
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        data_channel.on_message(Box::new(
            move |msg: DataChannelMessage| match RequestFrame::decode(&msg.data) {
                Ok(frame) => {
                    let tx = tx.clone();
                    Box::pin(async move {
                        if let Err(e) = tx.send(frame).await {
                            error!("failed to send message to receiver: {e}");
                        }
                    })
                },
                Err(e) => {
                    error!("failed to decode message: {e}");
                    Box::pin(async {})
                },
            },
        ));

        let receiver = WebRtcReceiver(rx);
        let socket = TokioSpawn::spawn_async(SendWorker(data_channel));
        let sender = WebRtcSender(socket);

        Some((req, sender, receiver))
    }
}

/// Sender for a webrtc connection.
pub struct WebRtcSender(Socket<Bytes, ()>);

macro_rules! webrtc_send {
    ($self:expr, $payload:expr) => {{
        let socket = $self.0.clone();
        futures::executor::block_on(async move {
            if let Err(e) = socket.enqueue($payload).await {
                error!("failed to send message to peer: {e}");
            };
        });
    }};
}

impl TransportSender for WebRtcSender {
    fn send_handshake_response(&mut self, frame: schema::HandshakeResponse) {
        webrtc_send!(self, frame.encode());
    }

    fn send(&mut self, frame: schema::ResponseFrame) {
        match frame {
            // Frames greater than the max payload size should be chunked with a novel frame tag
            // for webrtc.
            schema::ResponseFrame::ServicePayload { bytes } if bytes.len() > MAX_PAYLOAD_SIZE => {
                let mut iter = bytes.chunks(MAX_PAYLOAD_SIZE).peekable();
                while let Some(chunk) = iter.next() {
                    let mut payload = schema::ResponseFrame::ServicePayload {
                        bytes: chunk.to_vec().into(),
                    }
                    .encode()
                    .to_vec();

                    if iter.peek().is_some() {
                        // If we have more chunks to send, mark the frame tag as 0x40. This signals
                        // that the payload will have more data, and should be buffered and
                        // concatenated with the next frame. The last frame will retain the
                        // standard tag, 0x00, which also allows small payloads to be encoded
                        // normally.
                        payload[0] = 0x40;
                    }

                    webrtc_send!(self, payload.into())
                }
            },
            // Any other frames are encoded normally and sent out.
            _ => webrtc_send!(self, frame.encode()),
        };
    }
}

/// Receiver for a webrtc connection.
pub struct WebRtcReceiver(tokio::sync::mpsc::Receiver<schema::RequestFrame>);

#[async_trait]
impl TransportReceiver for WebRtcReceiver {
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        self.0.recv().await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use affair::{AsyncWorker, Executor, TokioSpawn};
    use async_trait::async_trait;
    use bytes::{BufMut, Bytes, BytesMut};
    use tokio::sync::mpsc::{channel, Sender};

    use crate::schema::ResponseFrame;
    use crate::transports::webrtc::{WebRtcSender, MAX_PAYLOAD_SIZE};
    use crate::transports::TransportSender;

    #[tokio::test]
    async fn sender_payload_chunking() {
        // Test payload that is the size of one of our complete blake3 blocks.
        const TEST_PAYLOAD: &[u8] = &[42; 256 << 10];

        /// Dummy worker that collects chunk frames and ensures the final length is correct.
        struct DummySendWorker(usize, BytesMut, Sender<Bytes>);
        #[async_trait]
        impl AsyncWorker for DummySendWorker {
            type Request = Bytes;
            type Response = ();

            async fn handle(&mut self, req: Self::Request) -> Self::Response {
                assert!(req.len() <= MAX_PAYLOAD_SIZE + 1);
                self.1.put(&req[1..]);

                match req[0] {
                    0x00 => {
                        println!("got final chunk");
                        assert_eq!(self.0, self.1.len());
                        self.2
                            .send(self.1.split().into())
                            .await
                            .expect("failed to send bytes");
                    },
                    0x40 => println!("got payload chunk"),
                    v => panic!("got {v}"),
                }
            }
        }

        // Init a notify channel, the worker and socket, and the webrtc sender.
        let (tx, mut rx) = channel(8);
        let socket = TokioSpawn::spawn_async(DummySendWorker(
            TEST_PAYLOAD.len(),
            BytesMut::with_capacity(TEST_PAYLOAD.len()),
            tx,
        ));
        let mut sender = WebRtcSender(socket);

        // Send the content
        sender.send(ResponseFrame::ServicePayload {
            bytes: TEST_PAYLOAD.into(),
        });

        // Wait for the worker to buffer and receive the entire payload
        let bytes = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout, did worker panic?")
            .expect("failed to recv bytes");
        assert_eq!(&bytes, TEST_PAYLOAD);
    }
}
