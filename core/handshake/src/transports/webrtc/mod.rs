mod driver;
mod signal;

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use anyhow::Result;
use async_trait::async_trait;
use axum::Router;
use bytes::Bytes;
use dashmap::DashMap;
use lightning_interfaces::prelude::*;
use lightning_metrics::increment_counter;
use serde::{Deserialize, Serialize};
use stunclient::StunClient;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::Notify;
use tracing::{debug, info, warn};
use triomphe::Arc;

use self::driver::{ConnectionMap, WebRtcDriver};
use super::{Transport, TransportReceiver, TransportSender};
use crate::schema::{self, HandshakeRequestFrame, RequestFrame};
use crate::transports::webrtc::signal::router;

/// WebRTC has a maximum payload size of ~64KiB, so we go a little under to be sure it's okay.
pub const MAX_PAYLOAD_SIZE: usize = 48 << 10;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct WebRtcConfig {
    pub address: SocketAddr,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4320).into(),
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
        info!("Binding WebRTC transport on {}", config.address);
        let conns = Arc::new(DashMap::new());

        // Bind the driver to the udp socket
        let driver = WebRtcDriver::bind(config.address, conns.clone()).await?;

        let stun = StunClient::new(
            "stun.l.google.com:19302"
                .to_socket_addrs()
                .unwrap()
                .find(|x| x.is_ipv4())
                .expect("failed to get stun server"),
        );
        let public_addr = stun.query_external_address_async(&driver.socket).await?;
        debug!("Got public address via STUN: {public_addr}");

        let mut local_addr = driver.socket.local_addr()?;
        if local_addr.ip() == IpAddr::from([0, 0, 0, 0]) {
            local_addr.set_ip(IpAddr::from([127, 0, 0, 1]));
        }

        // Spawn the IO loop
        let notify = Arc::new(Notify::new());
        spawn!(
            driver.run(waiter, notify.clone()),
            "HANDSHAKE: webrtc io loop"
        );

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
            current_write: 0,
        };
        let receiver = WebRtcReceiver(receiver);

        increment_counter!(
            "handshake_webrtc_sessions",
            Some("Counter for number of handshake sessions accepted over webrtc")
        );

        Some((req, sender, receiver))
    }
}

/// Sender for a webrtc connection.
pub struct WebRtcSender {
    addr: IpAddr,
    conns: ConnectionMap,
    notify: Arc<Notify>,
    current_write: usize,
}

impl WebRtcSender {
    #[inline(always)]
    fn send_inner(&mut self, payload: &[u8]) {
        if let Some(mut conn) = self.conns.get_mut(&self.addr) {
            if let Err(e) = conn.write(payload) {
                warn!("failed to write outgoing payload to rtc instance: {e}");
            }
            self.notify.notify_one();
        }
    }
}

impl TransportSender for WebRtcSender {
    #[inline(always)]
    async fn send_handshake_response(&mut self, frame: schema::HandshakeResponse) {
        self.send_inner(&frame.encode());
    }

    #[inline(always)]
    async fn send(&mut self, frame: schema::ResponseFrame) {
        debug_assert!(
            !matches!(
                frame,
                schema::ResponseFrame::ServicePayload { .. }
                    | schema::ResponseFrame::ServicePayloadChunk { .. }
            ),
            "write api should be used for service payloads"
        );

        // Other frames will never be larger than the max len, so we just send them.
        self.send_inner(&frame.encode());
    }

    #[inline(always)]
    async fn start_write(&mut self, len: usize) {
        debug_assert!(
            self.current_write == 0,
            "all bytes should be written before another call to start_write"
        );
        self.current_write = len;
    }

    // TODO: consider buffering up to the max payload size to send less chunks/extra bytes
    #[inline(always)]
    async fn write(&mut self, mut buf: Bytes) -> anyhow::Result<usize> {
        debug_assert!(self.current_write >= buf.len());

        while !buf.is_empty() {
            let amt = MAX_PAYLOAD_SIZE.min(buf.len());
            let bytes = buf.split_to(amt);

            if self.current_write > amt {
                // If we have more data to write for this service payload, send it as a chunk.
                self.send_inner(&schema::ResponseFrame::ServicePayloadChunk { bytes }.encode())
            } else {
                // If this is the last data, send it as a normal payload.
                self.send_inner(&schema::ResponseFrame::ServicePayload { bytes }.encode())
            }

            self.current_write -= amt;
        }

        Ok(0)
    }
}

/// Receiver for a webrtc connection.
pub struct WebRtcReceiver(Receiver<schema::RequestFrame>);

impl TransportReceiver for WebRtcReceiver {
    #[inline(always)]
    async fn recv(&mut self) -> Option<schema::RequestFrame> {
        self.0.recv().await
    }
}
