use affair::{AsyncWorker, Executor, TokioSpawn};
use anyhow::Result;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
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
    /// Receive requests to perform on connection.
    service_request_rx: Receiver<DriverRequest>,
    /// Send events from this connection.
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
    // The first stream is used for broadcasting messages.
    let (stream_tx, stream_rx) = match ctx.incoming {
        true => ctx.connection.accept_bi().await?,
        false => ctx.connection.open_bi().await?,
    };

    // Since streams are not cloneable, we spawn a worker to drive the sending side of the stream.
    let message_stream_tx = FramedWrite::new(stream_tx, LengthDelimitedCodec::new());
    let message_sender_socket = TokioSpawn::spawn_async(MessageSender {
        tx: message_stream_tx,
    });

    // We keep the receiving side of that stream.
    let mut message_stream_rx = FramedRead::new(stream_rx, LengthDelimitedCodec::new());

    loop {
        tokio::select! {
            accept_result = ctx.connection.accept_bi() => {
                let (stream_tx, stream_rx) = accept_result?;
                let connection_event_tx = ctx.connection_event_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_incoming_streams((stream_tx, stream_rx), connection_event_tx).await
                    {
                        tracing::error!("failed to handle incoming stream: {e:?}");
                    }
                });
            }
            incoming = message_stream_rx.next() => {
                let message = match incoming {
                    None => break,
                    Some(message) => message?,
                };
                let event_tx = ctx.connection_event_tx.clone();
                tokio::spawn(async move{
                    if let Err(e) = handle_incoming_message(message, event_tx).await {
                        tracing::error!("failed to send message: {e:?}");
                    }
                });
            }
            driver_request = ctx.service_request_rx.recv() => {
                match driver_request {
                    Some(DriverRequest::Message(message)) => {
                        // We send the message to the worker.
                        let msg_sender_tx = message_sender_socket.clone();
                        tokio::spawn(async move{
                            if let Err(e) = msg_sender_tx.run(message).await {
                                tracing::error!("failed to send message: {e:?}");
                            }
                        });
                    },
                    Some(DriverRequest::NewStream { service, respond }) => {
                        // We need to create a new stream on the connection.
                        let connection = ctx.connection.clone();
                        tokio::spawn(async move {
                            if let Err(e) = create_stream(connection, service, respond).await {
                                tracing::error!("failed to create stream: {e:?}");
                            }
                        });
                    }
                    None => break,
                }
            }
        }
    }
    Ok(())
}

async fn handle_incoming_streams(
    (stream_tx, mut stream_rx): (SendStream, RecvStream),
    connection_event_tx: Sender<ConnectionEvent>,
) -> Result<()> {
    // The peer opened a stream.
    // The first byte identifies the service.
    let mut buf = [0u8; 1];
    stream_rx.read_exact(&mut buf).await?;
    let service_scope = ServiceScope::try_from(buf[0])?;
    connection_event_tx
        .send(ConnectionEvent::Stream {
            service_scope,
            stream: (stream_tx, stream_rx),
        })
        .await
        .map_err(|_| anyhow::anyhow!("failed to send incoming network event"))
}

async fn handle_incoming_message(
    message: BytesMut,
    connection_event_tx: Sender<ConnectionEvent>,
) -> Result<()> {
    let message = Message::try_from(message)?;
    connection_event_tx
        .send(ConnectionEvent::Broadcast { message })
        .await
        .map_err(|_| anyhow::anyhow!("failed to send incoming network event"))
}

async fn create_stream(
    connection: Connection,
    service: ServiceScope,
    respond: oneshot::Sender<Result<(SendStream, RecvStream), ConnectionError>>,
) -> Result<()> {
    // We open a new QUIC stream.
    let (mut stream_tx, stream_rx) = connection.open_bi().await?;
    // We send the service scope first so
    // that the receiver knows the service
    // that this stream belongs to.
    stream_tx.write_all(&[service as u8]).await?;
    respond
        .send(Ok((stream_tx, stream_rx)))
        .map_err(|e| anyhow::anyhow!("failed to send new stream to client: {e:?}"))
}

/// Worker to send messages over the sending side of a stream.
///
/// Takes ownership of a non-cloneable stream.
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

/// Requests for a driver worker.
pub enum DriverRequest {
    Message(Message),
    NewStream {
        service: ServiceScope,
        respond: oneshot::Sender<Result<(SendStream, RecvStream), ConnectionError>>,
    },
}
