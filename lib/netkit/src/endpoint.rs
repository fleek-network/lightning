use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use quinn::Connection;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

type Message = Vec<u8>;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
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
    /// Ongoing dialing task futures.
    ongoing_dial: FuturesUnordered<BoxFuture<'static, (NodePublicKey, Result<Connection>)>>,
    /// Pending dialing tasks.
    pending_dial: HashMap<NodeAddress, CancellationToken>,
}

impl Endpoint {
    pub fn new(endpoint: quinn::Endpoint, request_rx: Receiver<Request>) -> Self {
        Self {
            endpoint,
            request_rx,
            pending_send: HashMap::new(),
            driver: HashMap::new(),
            ongoing_dial: FuturesUnordered::new(),
            pending_dial: HashMap::new(),
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
                    self.handle_request(request);
                }
                Some((peer_pk, connection_result)) = self.ongoing_dial.next() => {
                    match connection_result {
                        Ok(connection) => self.handle_connection(peer_pk, connection),
                        Err(e) => tracing::warn!("failed to connect to {peer_pk:?}: {e:?}"),
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_connection(&mut self, peer: NodePublicKey, connection: Connection) {
        todo!()
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
                    self.enqueue_dial_task(peer);
                }
            },
            Request::Metrics { .. } => todo!(),
        }
    }

    fn enqueue_dial_task(&mut self, address: NodeAddress) {
        let cancel = CancellationToken::new();
        self.pending_dial.insert(address, cancel.clone());
        let endpoint = self.endpoint.clone();
        let fut = async move {
            // Todo: Add config.
            let connect = || async move {
                endpoint
                    .connect(address.socket_address, "localhost")?
                    .await
                    .map_err(Into::into)
            };
            let connection = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(anyhow::anyhow!("dial was cancelled")),
                connection = connect() => connection,
            };
            (address.pk, connection)
        }
        .boxed();
        self.ongoing_dial.push(fut);
    }
}
