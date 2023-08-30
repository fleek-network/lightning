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
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::driver;

pub type Message = Vec<u8>;

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
    /// Ongoing drivers
    driver_set: JoinSet<NodePublicKey>,
    /// Pending outgoing messages.
    pending_send: HashMap<NodePublicKey, Vec<Message>>,
    /// Input requests for the endpoint.
    request_rx: Receiver<Request>,
    /// QUIC endpoint.
    endpoint: quinn::Endpoint,
    /// Ongoing dialing task futures.
    ongoing_dial: FuturesUnordered<BoxFuture<'static, (NodePublicKey, Result<Connection>)>>,
    /// Pending dialing tasks.
    pending_dial: HashMap<NodePublicKey, CancellationToken>,
}

impl Endpoint {
    pub fn new(endpoint: quinn::Endpoint, request_rx: Receiver<Request>) -> Self {
        Self {
            endpoint,
            request_rx,
            pending_send: HashMap::new(),
            driver: HashMap::new(),
            driver_set: JoinSet::new(),
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
                        Ok(connection) => self.handle_connection(peer_pk, connection, false),
                        Err(e) => tracing::warn!("failed to connect to {peer_pk:?}: {e:?}"),
                    }
                }
                Some(peer) = self.driver_set.join_next() => {
                    match peer {
                        Ok(pk) => {
                            self.driver.remove(&pk);
                        }
                        Err(e) => {
                            tracing::warn!("unable to clean up failed driver tasks: {e:?}");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_connection(&mut self, peer: NodePublicKey, connection: Connection, accept: bool) {
        // Todo: expose this to clients.
        // Todo: Pass this to driver task to let us know that we need to remove this from driver
        // map.
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (message_tx, message_rx) = mpsc::channel(1024);
        self.cancel_dial(&peer);
        self.driver.insert(peer, message_tx.clone());
        self.driver_set.spawn(async move {
            if let Err(e) = driver::start_driver(connection, message_rx, event_tx, accept).await {
                tracing::error!("driver for connection with {peer:?} shutdowned: {e:?}")
            }
            peer
        });
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
        self.pending_dial.insert(address.pk, cancel.clone());
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

    fn cancel_dial(&mut self, peer: &NodePublicKey) {
        if let Some(cancel) = self.pending_dial.remove(peer) {
            cancel.cancel();
        }
    }
}
