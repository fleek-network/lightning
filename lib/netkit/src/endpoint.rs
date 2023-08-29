use std::collections::HashMap;
use std::net::SocketAddr;

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

type Message = Vec<u8>;

#[derive(Eq, Hash, PartialEq)]
pub struct NodeAddress {
    pub pk: NodePublicKey,
    pub socket_address: SocketAddr,
}

pub enum Request {
    SendMessage {
        /// Peer to connect to.
        peer: NodeAddress,
        /// The outgoing message.
        message: Message,
        /// Respond with connection information such as
        /// negotiated parameters during handshake and peer IP.
        conn_info_tx: Sender<()>,
    },
    Metrics {
        /// Connection peer.
        peer: NodePublicKey,
        /// Channel to respond on with metrics.
        respond: oneshot::Sender<()>,
    },
}

pub struct Endpoint {
    /// Used for sending outbound messages to drivers.
    driver: HashMap<NodePublicKey, Sender<Message>>,
    /// Pending outgoing messages.
    pending_send: HashMap<NodePublicKey, Vec<Message>>,
    /// Input requests for the endpoint.
    request_rx: Receiver<Request>,
    /// QUIC endpoint.
    endpoint: quinn::Endpoint,
}

impl Endpoint {
    pub fn new(endpoint: quinn::Endpoint, request_rx: Receiver<Request>) -> Self {
        Self {
            endpoint,
            request_rx,
            pending_send: HashMap::new(),
            driver: HashMap::new(),
        }
    }

    pub async fn start(mut self) -> Result<()> {
        loop {
            tokio::select! {
                request = self.request_rx.recv() => {
                    let request = match request {
                        None => break,
                        Some(request) => request,
                    };

                }
            }
        }
        Ok(())
    }

    fn handle_request(&mut self, request: Request) {
        match request {
            Request::SendMessage { peer, message, .. } => {
                if self
                    .driver
                    .get(&peer.pk)
                    .map(|driver_tx| !driver_tx.is_closed())
                    .unwrap_or(false)
                {
                    let driver_tx = self.driver.get(&peer.pk).cloned().unwrap();
                    tokio::spawn(async move {
                        if driver_tx.send(message).await.is_err() {
                            tracing::error!("driver dropped unexpectedly");
                        }
                    });
                } else {
                    self.pending_send.entry(peer.pk).or_default().push(message);
                }
            },
            Request::Metrics { .. } => todo!(),
        }
    }
}
