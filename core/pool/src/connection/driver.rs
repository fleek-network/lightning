use affair::{AsyncWorker, Executor, TokioSpawn};
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use lightning_interfaces::ServiceScope;
use quinn::{Connection, ConnectionError, RecvStream, SendStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::connection::ConnectionEvent;
use crate::service::broadcast::Message;

/// Context for driving the connection.
pub struct Context {
    /// The QUIC connection.
    connection: Connection,
    /// Receive requests from services.
    service_request_rx: Receiver<DriverRequest>,
    /// Send events from the network.
    connection_event_tx: Sender<ConnectionEvent>,
    /// If the connection was started
    /// by the peer, this is true and false otherwise.
    incoming: bool,
}

impl Context {
    pub fn new(
        connection: Connection,
        service_request_rx: Receiver<DriverRequest>,
        connection_event_tx: Sender<ConnectionEvent>,
        incoming: bool,
    ) -> Self {
        Self {
            connection,
            service_request_rx,
            connection_event_tx,
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
    let message_stream_tx = FramedWrite::new(stream_tx, LengthDelimitedCodec::new());
    let message_sender_socket = TokioSpawn::spawn_async(MessageSender {
        tx: message_stream_tx,
    });
    let mut message_stream_rx = FramedRead::new(stream_rx, LengthDelimitedCodec::new());

    loop {
        tokio::select! {
            accept_bi = ctx.connection.accept_bi() => {
                let (tx, rx) = accept_bi?;
                let connection_event_tx = ctx.connection_event_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_incoming_streams((tx, rx), connection_event_tx).await
                    {
                        tracing::error!("failed to handle incoming stream: {e:?}");
                    }
                });
            }
            request = ctx.service_request_rx.recv() => {
                match request {
                    None => break,
                    Some(DriverRequest::Message(message)) => {
                        let tx = message_sender_socket.clone();
                        tokio::spawn(async move{
                            if let Err(e) = tx.run(message).await {
                                tracing::error!("failed to send message: {e:?}");
                            }
                        });
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
                        let connection_event_tx = ctx.connection_event_tx.clone();
                        tokio::spawn(async move {
                            if connection_event_tx
                                .send(ConnectionEvent::Broadcast{ message })
                                .await
                                .is_err() {
                                    tracing::error!("failed to send incoming network event");
                            }
                        });
                    },
                    Err(e) => tracing::error!("failed to deserialize message: {e:?}"),
                }
            }
        }
    }
    Ok(())
}

async fn handle_incoming_streams(
    (_, mut rx): (SendStream, RecvStream),
    connection_event_tx: Sender<ConnectionEvent>,
) -> Result<()> {
    // The peer opened a stream.
    // The first byte identifies the service.
    let mut buf = [0u8; 1];
    rx.read_exact(&mut buf).await?;
    let service_scope = ServiceScope::try_from(buf[0])?;
    if connection_event_tx
        .send(ConnectionEvent::Stream { service_scope })
        .await
        .is_err()
    {
        tracing::error!("failed to send incoming network event");
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
    tx.write_all(&[service as u8]).await?;
    respond
        .send(Ok((tx, rx)))
        .map_err(|e| anyhow::anyhow!("failed to send new stream to client: {e:?}"))
}

pub struct MessageSender {
    tx: FramedWrite<SendStream, LengthDelimitedCodec>,
}

#[async_trait]
impl AsyncWorker for MessageSender {
    type Request = Message;
    type Response = Result<()>;

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.tx.send(Bytes::from(req)).await.map_err(Into::into)
    }
}

pub enum DriverRequest {
    Message(Message),
    #[allow(unused)]
    NewStream {
        service: ServiceScope,
        respond: oneshot::Sender<Result<(SendStream, RecvStream), ConnectionError>>,
    },
}
