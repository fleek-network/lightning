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

pub async fn start_driver(
    connection: Connection,
    peer: NodePublicKey,
    mut service_request_rx: Receiver<DriverRequest>,
    service_event_tx: HashMap<ServiceScope, Sender<Event>>,
    accept: bool,
) -> Result<()> {
    // Todo: If we stick with QUIC, we should use the stream more efficiently.
    let (tx, rx) = match accept {
        true => connection.accept_bi().await?,
        false => connection.open_bi().await?,
    };
    let mut writer = FramedWrite::new(tx, LengthDelimitedCodec::new());
    let mut reader = FramedRead::new(rx, LengthDelimitedCodec::new());
    loop {
        tokio::select! {
            accept_bi = connection.accept_bi() => {
                match accept_bi {
                    Ok((tx, mut rx)) => {
                        let mut buf = [0u8; 1];
                        match rx.read_exact(&mut buf).await {
                            Ok(()) => {
                                let service = buf[0];
                                match service_event_tx.get(&service) {
                                    Some(sender) => {
                                        let event = Event::NewStream { peer, tx, rx };
                                        if sender.send(event).await.is_err() {
                                            anyhow::bail!("failed to send incoming network event");
                                        }
                                    },
                                    None => info!("received stream request with invalid service scope: {service}"),
                                }
                            },
                            Err(e) => error!("failed to read from stream: {e:?}"),
                        }
                    }
                    Err(e) => error!("failed to accept connection: {e:?}"),
                }
            }
            request = service_request_rx.recv() => {
                match request {
                    None => break,
                    Some(DriverRequest::Message(message)) => {
                        writer.send(Bytes::from(message)).await?
                    },
                    Some(DriverRequest::NewStream { service, respond }) => {
                        let open_bi = match connection.open_bi().await {
                            Ok((mut tx, rx)) => {
                                tx.write_all(&[service]).await?;
                                Ok((tx, rx))
                            }
                            Err(e) => Err(e),
                        };
                        if let Err(e) = respond.send(open_bi) {
                            anyhow::bail!("failed to stream: {e:?}");
                        }
                    }
                }
            }
            incoming = reader.next() => {
                let message = match incoming {
                    None => break,
                    Some(message) => message?,
                };
                match Message::try_from(message) {
                    Ok(message) => {
                        match service_event_tx.get(&message.service) {
                            Some(sender) => {
                                if sender.send(Event::Message{ peer, message }).await.is_err() {
                                    anyhow::bail!("failed to send incoming network event");
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
