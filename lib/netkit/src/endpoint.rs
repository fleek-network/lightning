use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use quinn::{ClientConfig, Connecting, Connection, ServerConfig};
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
    },
    Metrics {
        /// Connection peer.
        peer: NodePublicKey,
        /// Channel to respond on with metrics.
        respond: oneshot::Sender<()>,
    },
}

pub enum Event {
    Message {
        peer: NodePublicKey,
        message: Message,
    },
    NewConnection {
        peer: NodePublicKey,
        rtt: Duration,
    },
}

pub struct Endpoint {
    /// Socket address.
    address: SocketAddr,
    /// QUIC server config.
    server_config: ServerConfig,
    /// QUIC endpoint.
    endpoint: Option<quinn::Endpoint>,
    /// The node's key.
    sk: NodeSecretKey,
    /// Receiver for requests for the endpoint.
    request_rx: Receiver<Request>,
    /// Sender for requests for the endpoint.
    request_tx: Sender<Request>,
    /// Ongoing incoming and outgoing connection set-up tasks.
    connecting: FuturesUnordered<BoxFuture<'static, ConnectionResult>>,
    /// Pending dialing tasks.
    pending_dial: HashMap<NodePublicKey, CancellationToken>,
    /// Pending outgoing messages.
    pending_send: HashMap<NodePublicKey, Vec<Message>>,
    /// Used for sending outbound messages to drivers.
    driver: HashMap<NodePublicKey, Sender<Message>>,
    /// Ongoing drivers.
    driver_set: JoinSet<NodePublicKey>,
    /// Receiver for network events.
    network_event_tx: Sender<Event>,
    /// Receiver for network events.
    network_event_rx: Option<Receiver<Event>>,
}

impl Endpoint {
    pub fn new(sk: NodeSecretKey, address: SocketAddr, server_config: ServerConfig) -> Self {
        let (network_event_tx, network_event_rx) = mpsc::channel(1024);
        let (request_tx, request_rx) = mpsc::channel(1024);
        Self {
            address,
            endpoint: None,
            server_config,
            sk,
            request_tx,
            request_rx,
            pending_send: HashMap::new(),
            driver: HashMap::new(),
            driver_set: JoinSet::new(),
            connecting: FuturesUnordered::new(),
            pending_dial: HashMap::new(),
            network_event_tx,
            network_event_rx: Some(network_event_rx),
        }
    }

    /// Returns sender for requests to the endpoint.
    pub fn request_sender(&mut self) -> Sender<Request> {
        self.request_tx.clone()
    }

    /// Returns receiver for network events.
    ///
    /// Panics if called more than once.
    pub fn network_event_receiver(&mut self) -> Receiver<Event> {
        self.network_event_rx
            .take()
            .expect("To be called called once")
    }

    // Todo: Return metrics.
    pub async fn start(mut self) -> Result<()> {
        let endpoint = quinn::Endpoint::server(self.server_config.clone(), self.address)?;
        tracing::info!("bound to {:?}", endpoint.local_addr()?);

        self.endpoint.replace(endpoint.clone());

        loop {
            tokio::select! {
                request = self.request_rx.recv() => {
                    let request = match request {
                        None => break,
                        Some(request) => request,
                    };
                    if let Err(e) = self.handle_request(request) {
                        tracing::error!("failed to handle request: {e:?}");
                    }
                }
                connecting = endpoint.accept() => {
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
                            let peer = peer.unwrap();
                            tracing::warn!("failed to dial peer {:?}: {e:?}", peer);
                            self.remove_pending_dial(&peer);
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
        self.cancel_dial(&peer);

        let (message_tx, message_rx) = mpsc::channel(1024);
        self.driver.insert(peer, message_tx);

        let event_tx = self.network_event_tx.clone();
        self.driver_set.spawn(async move {
            if event_tx
                .send(Event::NewConnection {
                    peer,
                    rtt: connection.rtt(),
                })
                .await
                .is_err()
            {
                tracing::error!("endpoint client dropped the network event receiver");
            }

            if let Err(e) =
                driver::start_driver(connection, peer, message_rx, event_tx, accept).await
            {
                tracing::error!("driver for connection with {peer:?} shutdowned: {e:?}")
            }
            peer
        });
    }

    fn handle_request(&mut self, request: Request) -> Result<()> {
        match request {
            Request::SendMessage { peer, message } => {
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
                    self.enqueue_dial_task(peer)?;
                }
            },
            Request::Metrics { .. } => todo!(),
        }
        Ok(())
    }

    fn enqueue_dial_task(&mut self, address: NodeAddress) -> Result<()> {
        let cancel = CancellationToken::new();
        self.pending_dial.insert(address.pk, cancel.clone());

        let endpoint = self.endpoint.clone().expect("There to be an endpoint");
        let tls_config = tls::make_client_config(&self.sk, Some(address.pk))?;
        let fut = async move {
            let client_config = ClientConfig::new(Arc::new(tls_config));
            let connect = || async move {
                endpoint
                    .connect_with(client_config, address.socket_address, "localhost")?
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

        Ok(())
    }

    fn cancel_dial(&mut self, peer: &NodePublicKey) {
        if let Some(cancel) = self.pending_dial.remove(peer) {
            cancel.cancel();
        }
    }

    fn remove_pending_dial(&mut self, peer: &NodePublicKey) {
        self.pending_dial.remove(peer);
    }
}

struct ConnectionResult {
    pub accept: bool,
    pub conn: Result<Connection>,
    pub peer: Option<NodePublicKey>,
}
