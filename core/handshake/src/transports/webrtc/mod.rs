mod driver;
mod signal;

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use anyhow::Result;
use async_trait::async_trait;
use axum::Router;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use stunclient::StunClient;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::Notify;
use tracing::{debug, info, warn};
use triomphe::Arc;

use self::driver::{ConnectionMap, WebRtcDriver};
use super::{Transport, TransportReceiver, TransportSender};
use crate::schema::{self, HandshakeRequestFrame, RequestFrame};
use crate::shutdown::ShutdownWaiter;
use crate::transports::webrtc::signal::router;

/// WebRTC has a maximum payload size of ~64KiB, so we go a little under to be sure it's okay.
pub const MAX_PAYLOAD_SIZE: usize = 48 << 10;

#[derive(Serialize, Deserialize, Clone)]
pub struct WebRtcConfig {
    pub udp_address: SocketAddr,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            udp_address: ([0, 0, 0, 0], 4320).into(),
        }
    }
}

/// A WebRTC Transport. Spawns a HTTP signaling server, and binds to ephemeral UDP ports per
/// peer connection.
pub struct WebRtcTransport {
    /// Receiver for incoming DataChannels from peer connections.
    conn_rx: Receiver<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
    /// Lock-free shared map of connection states
    conns: ConnectionMap,
    /// Waker for the driver, to immediately poll connection states and send
    /// outgoing data over the socket.
    notify: Arc<Notify>,
}

#[async_trait]
impl Transport for WebRtcTransport {
    type Config = WebRtcConfig;
    type Sender = WebRtcSender;
    type Receiver = WebRtcReceiver;

    async fn bind(waiter: ShutdownWaiter, config: Self::Config) -> Result<(Self, Option<Router>)> {
        info!("Binding WebRTC transport on {}", config.udp_address);
        let conns = Arc::new(DashMap::new());

        // Bind the driver to the udp socket
        let driver = WebRtcDriver::bind(config.udp_address, conns.clone()).await?;

        let stun = StunClient::new(
            "stun.l.google.com:19302"
                .to_socket_addrs()
                .unwrap()
                .find(|x| x.is_ipv4())
                .expect("failed to get stun server"),
        );
        let mut local_addr = driver.socket.local_addr()?;
        if local_addr.ip() == IpAddr::from([0, 0, 0, 0]) {
            local_addr.set_ip(IpAddr::from([127, 0, 0, 1]));
        }

        let public_addr = stun.query_external_address_async(&driver.socket).await?;
        debug!("Got public address via STUN: {public_addr}");

        // Spawn the IO loop
        let notify = Arc::new(Notify::new());
        tokio::spawn(driver.run(waiter, notify.clone()));

        // A bounded channel is used to provide some back pressure for incoming client handshakes.
        let (conn_tx, conn_rx) = channel(1024);
        // Construct our http router for negotiating via SDP.
        let router = router(vec![public_addr, local_addr], conns.clone(), conn_tx)?;

        Ok((
            Self {
                conn_rx,
                conns,
                notify,
            },
            Some(router),
        ))
    }

    async fn accept(
        &mut self,
    ) -> Option<(schema::HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        let (req, addr, receiver) = self.conn_rx.recv().await?;

        let sender = WebRtcSender {
            addr,
            conns: self.conns.clone(),
            notify: self.notify.clone(),
        };
        let receiver = WebRtcReceiver(receiver);

        Some((req, sender, receiver))
    }
}

/// Sender for a webrtc connection.
pub struct WebRtcSender {
    addr: IpAddr,
    conns: ConnectionMap,
    notify: Arc<Notify>,
}

impl WebRtcSender {
    #[inline(always)]
    fn send(&mut self, payload: &[u8]) {
        if let Some(mut conn) = self.conns.get_mut(&self.addr) {
            if let Err(e) = conn.write(payload) {
                warn!("failed to write outgoing payload to rtc instance: {e}");
            }
            self.notify.notify_one();
        }
    }
}

impl TransportSender for WebRtcSender {
    fn send_handshake_response(&mut self, frame: schema::HandshakeResponse) {
        self.send(&frame.encode());
    }

    fn send(&mut self, frame: schema::ResponseFrame) {
        match frame {
            // Chunk outgoing payloads larger than the webrtc max
            schema::ResponseFrame::ServicePayload { bytes } if bytes.len() > MAX_PAYLOAD_SIZE => {
                let mut iter = bytes.chunks(MAX_PAYLOAD_SIZE).peekable();
                while let Some(chunk) = iter.next() {
                    let frame = schema::ResponseFrame::ServicePayload {
                        bytes: chunk.to_vec().into(),
                    };
                    let mut bytes = frame.encode().to_vec();

                    if iter.peek().is_some() {
                        // Set the frame tag to 0x40, signaling that it is a partial chunk of the
                        // service payload, and should be buffered and concatenated with
                        // the next chunk. The last chunk will keep the original frame tag (0x00),
                        // signaling that the payload is complete, which also allows single
                        // chunk payloads to use the normal frame tag.
                        bytes[0] = 0x40;
                    }

                    // Send the chunk
                    self.send(&bytes);
                }
            },
            frame => {
                // Other frames will never be larger than the max len, so we just send them.
                self.send(&frame.encode());
            },
        }
    }
}

/// Receiver for a webrtc connection.
pub struct WebRtcReceiver(Receiver<schema::RequestFrame>);

#[async_trait]
impl TransportReceiver for WebRtcReceiver {
    #[inline(always)]
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        self.0.recv().await
    }
}
