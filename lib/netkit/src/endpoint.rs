use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;

use anyhow::{Error, Result};
use fleek_crypto::NodePublicKey;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use quinn::{Connecting, Connection, ConnectionError};
use rustls::Certificate;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::{driver, tls};

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
    connecting: FuturesUnordered<BoxFuture<'static, ConnectionResult>>,
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
            connecting: FuturesUnordered::new(),
            pending_dial: HashMap::new(),
        }
    }
    // Todo: Return metrics.
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
                connecting = self.endpoint.accept() => {
                    match connecting {
                        None => break,
                        Some(connecting) => self.handle_incoming_connection(connecting),
                    }
                }
                Some(connection_result) = self.connecting.next() => {
                    let ConnectionResult { accept, conn, peer} = connection_result;
                    match conn {
                        Ok(connection) => {
                            // The unwrap here is safe because when accepting connections,
                            // we will fail to connect if we cannot obtain the peer's
                            // public key from the TLS session. When dialing, we already
                            // have the peer's public key.
                            self.handle_connection(peer.unwrap(), connection, accept)
                        }
                        Err(e) if accept => {
                            tracing::warn!("failed to connect to peer: {e:?}");
                        }
                        Err(e) => {
                            // The unwrap here is safe. See comment above.
                            tracing::warn!("failed to dial peer {:?}: {e:?}", peer.unwrap());
                        }
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

    fn handle_incoming_connection(&mut self, connecting: Connecting) {
        let fut = async move {
            let connect = || async move {
                let connection = connecting.await?;
                let key = match connection.peer_identity() {
                    None => {
                        anyhow::bail!("failed to get peer identity from successful TLS handshake")
                    },
                    Some(any) => {
                        let chain = any
                            .downcast::<Vec<Certificate>>()
                            .map_err(|_| anyhow::anyhow!("invalid peer certificate"))?;
                        let certificate = chain
                            .first()
                            .ok_or_else(|| anyhow::anyhow!("invalid certificate chain"))?;
                        tls::parse_unverified(certificate.as_ref())?.peer_pk()
                    },
                };
                Ok((key, connection))
            };
            match connect().await {
                Ok((key, conn)) => ConnectionResult {
                    accept: true,
                    conn: Ok(conn),
                    peer: Some(key),
                },
                Err(e) => ConnectionResult {
                    accept: true,
                    conn: Err(e),
                    peer: None,
                },
            }
        }
        .boxed();
        self.connecting.push(fut);
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
            ConnectionResult {
                accept: false,
                conn: connection,
                peer: Some(address.pk),
            }
        }
        .boxed();
        self.connecting.push(fut);
    }

    fn cancel_dial(&mut self, peer: &NodePublicKey) {
        if let Some(cancel) = self.pending_dial.remove(peer) {
            cancel.cancel();
        }
    }
}

struct ConnectionResult {
    pub accept: bool,
    pub conn: Result<Connection>,
    pub peer: Option<NodePublicKey>,
}
