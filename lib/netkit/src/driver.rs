use std::collections::HashMap;

use anyhow::Result;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use futures::{SinkExt, StreamExt};
use log::{error, info};
use quinn::{Connection, ConnectionError, RecvStream, SendStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::endpoint::{DriverRequest, Event, Message, ServiceScope};

/// Context for driving the connection.
pub struct Context {
    /// The QUIC connection.
    connection: Connection,
    /// The peer's public key.
    peer: NodePublicKey,
    /// Receive requests from services.
    service_request_rx: Receiver<DriverRequest>,
    /// Send events from the network.
    network_event_tx: HashMap<ServiceScope, Sender<Event>>,
    /// If the connection was started
    /// by the peer, this is true and false otherwise.
    incoming: bool,
}

impl Context {
    pub fn new(
        connection: Connection,
        peer: NodePublicKey,
        service_request_rx: Receiver<DriverRequest>,
        service_event_tx: HashMap<ServiceScope, Sender<Event>>,
        incoming: bool,
    ) -> Self {
        Self {
            connection,
            peer,
            service_request_rx,
            network_event_tx: service_event_tx,
            incoming,
        }
    }
}

/// Drives the connection.
pub async fn start_driver(mut ctx: Context) -> Result<()> {
    let (stream_tx, stream_rx) = match ctx.incoming {
        true => ctx.connection.accept_bi().await?,
        false => ctx.connection.open_bi().await?,
    };

    // The first stream is used for sending messages.
    // This stream is used by all services that wish to send messages.
    let mut message_stream_tx = FramedWrite::new(stream_tx, LengthDelimitedCodec::new());
    let mut message_stream_rx = FramedRead::new(stream_rx, LengthDelimitedCodec::new());

    loop {
        tokio::select! {
            accept_bi = ctx.connection.accept_bi() => {
                let (tx, rx) = accept_bi?;
                let network_event_tx = ctx.network_event_tx.clone();
                let peer = ctx.peer;
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_incoming_streams((tx, rx), network_event_tx, peer).await
                    {
                        tracing::error!("failed to handle incoming stream: {e:?}");
                    }
                });
            }
            request = ctx.service_request_rx.recv() => {
                match request {
                    None => break,
                    Some(DriverRequest::Message(message)) => {
                        // Todo: make this to a worker and use a socket here.
                        // to avoid waiting on this main loop.
                        message_stream_tx.send(Bytes::from(message)).await?
                    },
                    Some(DriverRequest::NewStream { service, respond }) => {
                        let connection = ctx.connection.clone();
                        tokio::spawn(async move {
                            if let Err(e) = create_stream(connection, respond, service).await {
                                tracing::error!("failed to create stream: {e:?}");
                            }
                        });
                    }
                }
            }
            incoming = message_stream_rx.next() => {
                let message = match incoming {
                    None => break,
                    Some(message) => message?,
                };
                match Message::try_from(message) {
                    Ok(message) => {
                        match ctx.network_event_tx.get(&message.service).cloned() {
                            Some(sender) => {
                                let peer = ctx.peer;
                                tokio::spawn(async move {
                                    if sender
                                        .send(Event::Message{ peer, message })
                                        .await
                                        .is_err() {
                                            tracing::error!("failed to send incoming network event");
                                        }
                                });

                            },
                            None => info!("received stream request with invalid service scope: {}", message.service),
                        }
                    },
                    Err(e) => error!("failed to deserialize message: {e:?}"),
                }
            }
        }
    }
    Ok(())
}

async fn handle_incoming_streams(
    (tx, mut rx): (SendStream, RecvStream),
    network_event_tx: HashMap<ServiceScope, Sender<Event>>,
    peer: NodePublicKey,
) -> Result<()> {
    // The peer opened a stream.
    // The first byte identifies the service.
    let mut buf = [0u8; 1];
    rx.read_exact(&mut buf).await?;
    let service = buf[0];
    match network_event_tx.get(&service) {
        Some(sender) => {
            let event = Event::NewStream { peer, tx, rx };
            if sender.send(event).await.is_err() {
                tracing::error!("failed to send incoming network event");
            }
        },
        None => info!("received stream request with invalid service scope: {service}"),
    }
    Ok(())
}

async fn create_stream(
    connection: Connection,
    respond: oneshot::Sender<Result<(SendStream, RecvStream), ConnectionError>>,
    service: ServiceScope,
) -> Result<()> {
    // We open a new QUIC stream.
    let (mut tx, rx) = connection.open_bi().await?;
    // We send the service scope first so
    // that the receiver knows the service
    // that this stream belongs to.
    tx.write_all(&[service]).await?;
    respond
        .send(Ok((tx, rx)))
        .map_err(|e| anyhow::anyhow!("failed to send new stream to client: {e:?}"))
}
