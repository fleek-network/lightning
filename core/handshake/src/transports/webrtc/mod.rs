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
    ($self:expr, $frame:expr) => {
        let socket = $self.0.clone();
        let bytes = $frame.encode();
        futures::executor::block_on(async move {
            if let Err(e) = socket.enqueue(bytes).await {
                error!("failed to send message to peer: {e}");
            };
        });
    };
}

impl TransportSender for WebRtcSender {
    fn send_handshake_response(&mut self, frame: schema::HandshakeResponse) {
        webrtc_send!(self, frame);
    }

    fn send(&mut self, frame: schema::ResponseFrame) {
        webrtc_send!(self, frame);
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
