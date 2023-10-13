use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use str0m::channel::{ChannelData, ChannelId};
use str0m::net::{Receive, Transmit};
use str0m::{Event, IceConnectionState, Input, Output, Rtc};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Notify;
use tracing::{error, trace, warn};
use triomphe::Arc;

use crate::schema::{HandshakeRequestFrame, RequestFrame};
use crate::shutdown::ShutdownWaiter;

/// Driver for our webrtc server.
pub(crate) struct WebRtcDriver {
    /// Underlying UDP socket used across all connections.
    pub(crate) socket: UdpSocket,
    /// Map holding client connection and IO states.
    conns: ConnectionMap,
}

impl WebRtcDriver {
    pub async fn bind(udp_addr: SocketAddr, conns: ConnectionMap) -> Result<Self> {
        // Bind to the udp socket
        let socket = UdpSocket::bind(udp_addr).await?;

        Ok(Self { socket, conns })
    }

    pub async fn run(self, shutdown_waiter: ShutdownWaiter, notify: Arc<Notify>) {
        // UDP datagrams should be ~2KB max
        let buf = &mut [0; 2 << 10];

        while !shutdown_waiter.is_shutdown() {
            // Remove any disconnected clients.
            self.conns.retain(|_, c| c.rtc.is_alive());

            // Poll client states until they all are at an idle state.
            // TODO: Move client event loops into their own tokio tasks,
            //       driven separately from the socket itself, and utilize
            //       message passing.
            let mut timeouts = Vec::with_capacity(self.conns.len());
            for mut client in self.conns.iter_mut() {
                // Drive the state until it's idle
                match client.poll_until_idle(&self.socket).await {
                    Ok(t) => timeouts.push(t),
                    Err(e) => warn!("failed to drive client rtc state: {e}"),
                }
            }

            let now = Instant::now();
            let timeout = timeouts
                .into_iter()
                .min()
                .map(|t| now.saturating_duration_since(t))
                .unwrap_or(Duration::from_millis(5000))
                .max(Duration::from_millis(1));

            tokio::select! {
                // Read incoming data from the socket
                res = self.socket.recv_from(buf) => {
                    match res {
                        Ok((n, source)) => {
                            // Get the connection state for this address and handle the input
                            if let Some(mut conn) = self.conns.get_mut(&source.ip()) {
                                if let Err(e) = conn.handle_input(Input::Receive(
                                    Instant::now(),
                                    Receive {
                                        source,
                                        destination: self.socket.local_addr().unwrap(),
                                        contents: buf[..n].try_into().unwrap(),
                                    },
                                )) {
                                    warn!("failed to handle client input: {e}");
                                };
                            };
                        },
                        Err(e) => {
                            error!("Failed to read from udp socket: {e}");
                        },
                    };
                }
                // Timeout in case a connection needs be driven and possibly enter a disconnected state
                _ = tokio::time::sleep(timeout) => {}
                // Waker for outgoing data written to the state
                _ = notify.notified() => {}
            }

            // Drive time forward in all clients.
            let now = Instant::now();
            for mut conn in self.conns.iter_mut() {
                if let Err(e) = conn.handle_input(Input::Timeout(now)) {
                    warn!("failed to advance time for rtc instance: {e}");
                }
            }
        }
    }
}

/// Map of clients. Currently, a client can only have a single webrtc connection.
pub(crate) type ConnectionMap = Arc<DashMap<IpAddr, Connection>>;

/// Connection object holding the rtc instance and the handshake state
pub struct Connection {
    rtc: Rtc,
    addr: IpAddr,
    state: ConnectionState,
    conn_tx: Sender<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
}

/// Connection states for a client
enum ConnectionState {
    /// Waiting for the first data channel
    AwaitingDataChannel,
    /// Waiting for the handshake frame
    AwaitingHandshake(ChannelId),
    /// Main loop, waiting for a client request frame
    AwaitingRequest(ChannelId, Sender<RequestFrame>),
    /// Disconnected network connection
    Disconnected,
}

impl Connection {
    pub fn new(
        rtc: Rtc,
        addr: IpAddr,
        conn_tx: Sender<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
    ) -> Self {
        Self {
            rtc,
            addr,
            state: ConnectionState::AwaitingDataChannel,
            conn_tx,
        }
    }

    /// Helper to handle external input in the rtc state (socket data, timeouts)
    #[inline(always)]
    fn handle_input(&mut self, input: Input) -> Result<()> {
        self.rtc.handle_input(input)?;
        Ok(())
    }

    /// Poll and handle output until the rtc state is idle and returns a timeout
    #[inline(always)]
    async fn poll_until_idle(&mut self, socket: &UdpSocket) -> Result<Instant> {
        loop {
            let output = self.rtc.poll_output()?;
            match self.handle_output(socket, output).await? {
                None => continue,
                Some(t) => return Ok(t),
            }
        }
    }

    /// Handle output from the rtc state. On error, the connection should be
    /// considered disconnected. If the state is idle, returns an instant when the connection will
    /// time out.
    #[inline(always)]
    async fn handle_output(
        &mut self,
        socket: &UdpSocket,
        output: Output,
    ) -> Result<Option<Instant>> {
        match output {
            // The rtc is at an idle state, and is waiting for input
            Output::Timeout(t) => {
                return Ok(Some(t));
            },
            // We have some data to send on the socket.
            Output::Transmit(Transmit {
                destination,
                contents,
                ..
            }) => {
                let payload: &[u8] = contents.as_ref();
                let len = payload.len();
                let sent = socket.send_to(payload, destination).await?;
                debug_assert_eq!(len, sent);
            },
            // We have an incoming event
            Output::Event(event) => self.handle_event(event).await?,
        }

        Ok(None)
    }

    /// Handle a rtc event
    #[inline(always)]
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            // A channel has been opened.
            // TODO: Support multiple channels as top level "connections"
            Event::ChannelOpen(id, label) => {
                if let ConnectionState::AwaitingDataChannel = self.state {
                    trace!("Client opened active channel {label}");
                    // TODO: Send challenge key for client proof of possession

                    // Tick the handshake state forward
                    self.state = ConnectionState::AwaitingHandshake(id);
                }
            },
            // Message has been received
            Event::ChannelData(msg) => self.handle_message(msg).await?,
            // Connection has disconnected
            Event::IceConnectionStateChange(state) if state == IceConnectionState::Disconnected => {
                trace!("Client disconnected");
                self.state = ConnectionState::Disconnected;
            },
            // Channel has been closed
            Event::ChannelClose(id) => {
                // If the active channel gets closed, it should effectively reset the connection
                // state to allow for a new active channel to be opened.
                if let ConnectionState::AwaitingHandshake(current_id)
                | ConnectionState::AwaitingRequest(current_id, _) = self.state
                {
                    if id == current_id {
                        trace!("Client closed active data-channel");
                        // Reset the connection state
                        self.state = ConnectionState::AwaitingDataChannel;
                    }
                }
            },
            _ => {},
        }

        Ok(())
    }

    /// Handle an incoming payload
    #[inline(always)]
    async fn handle_message(&mut self, msg: ChannelData) -> Result<()> {
        if msg.binary {
            match &self.state {
                ConnectionState::AwaitingHandshake(id) if id == &msg.id => {
                    let frame = HandshakeRequestFrame::decode(&msg.data)?;

                    // Send the new connection to the transport
                    let (tx, rx) = channel(256);
                    self.conn_tx.send((frame, self.addr, rx)).await?;

                    // Tick the handshake state forward
                    self.state = ConnectionState::AwaitingRequest(*id, tx);
                },
                ConnectionState::AwaitingRequest(id, tx) if id == &msg.id => {
                    let frame = RequestFrame::decode(&msg.data)?;
                    tx.send(frame).await?;
                },
                ConnectionState::AwaitingDataChannel => {
                    unreachable!("will never have data before a channel opens")
                },
                ConnectionState::Disconnected => {
                    unreachable!("should never get messages after a disconnect")
                },
                _ => {},
            }
        }

        Ok(())
    }

    /// Write an outgoing payload to the rtc instances data channel.
    /// Should only be called with payloads < 64KB
    #[inline(always)]
    pub(super) fn write(&mut self, payload: &[u8]) -> Result<()> {
        match self.state {
            ConnectionState::AwaitingDataChannel => Err(anyhow!(
                "data channel has not been opened by the client yet"
            )),
            ConnectionState::AwaitingHandshake(id) | ConnectionState::AwaitingRequest(id, _) => {
                let mut channel = self
                    .rtc
                    .channel(id)
                    .ok_or(anyhow!("failed to get channel writer"))?;

                let written = channel.write(true, payload)?;
                debug_assert_eq!(payload.len(), written);

                Ok(())
            },
            ConnectionState::Disconnected => Err(anyhow!("connection state is disconnected")),
        }
    }
}
