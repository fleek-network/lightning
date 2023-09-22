use std::collections::HashMap;

use anyhow::Result;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use futures::{SinkExt, StreamExt};
use log::{error, info};
use quinn::Connection;
use tokio::sync::mpsc::{Receiver, Sender};
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
                // The peer opened a stream.
                // The first byte identifies the service.
                let (tx, mut rx) = accept_bi?;
                let mut buf = [0u8; 1];
                rx.read_exact(&mut buf).await?;
                let service = buf[0];
                match ctx.network_event_tx.get(&service) {
                    Some(sender) => {
                        let event = Event::NewStream { peer: ctx.peer, tx, rx };
                        if sender.send(event).await.is_err() {
                            tracing::error!("failed to send incoming network event");
                        }
                    },
                    None => info!("received stream request with invalid service scope: {service}"),
                }
            }
            request = ctx.service_request_rx.recv() => {
                match request {
                    None => break,
                    Some(DriverRequest::Message(message)) => {
                        message_stream_tx.send(Bytes::from(message)).await?
                    },
                    Some(DriverRequest::NewStream { service, respond }) => {
                        // We open a new QUIC stream.
                        let (mut tx, rx) =  ctx.connection.open_bi().await?;
                        // We send the service scope first so
                        // that the receiver knows the service
                        // that this stream belongs to.
                        tx.write_all(&[service]).await?;
                        if let Err(e) = respond.send(Ok((tx, rx))) {
                            tracing::error!("failed to send new stream to client: {e:?}");
                        }
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
                        match ctx.network_event_tx.get(&message.service) {
                            Some(sender) => {
                                if sender
                                .send(Event::Message{ peer: ctx.peer, message })
                                .await
                                .is_err() {
                                    tracing::error!("failed to send incoming network event");
                                }
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
