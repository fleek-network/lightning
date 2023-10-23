use std::io;

use anyhow::Result;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::ServiceScope;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::endpoint::ConnectionEvent;
use crate::muxer::{Channel, ConnectionInterface};
use crate::overlay::Message;

/// Context for driving the connection.
pub struct Context<C> {
    /// The multiplexed connection.
    connection: C,
    /// The peer's index.
    peer: NodeIndex,
    /// Receive requests to perform on connection.
    service_request_rx: Receiver<DriverRequest>,
    /// Send events from this connection.
    connection_event_tx: Sender<ConnectionEvent>,
}

impl<C: ConnectionInterface> Context<C> {
    pub fn new(
        connection: C,
        peer: NodeIndex,
        service_request_rx: Receiver<DriverRequest>,
        connection_event_tx: Sender<ConnectionEvent>,
    ) -> Self {
        Self {
            connection,
            peer,
            service_request_rx,
            connection_event_tx,
        }
    }
}

pub async fn start_driver<C: ConnectionInterface>(mut ctx: Context<C>) -> Result<()> {
    let mut connection = ctx.connection.clone();
    loop {
        tokio::select! {
            accept_result = ctx.connection.accept_stream() => {
                let (stream_tx, stream_rx) = accept_result?;
                let connection_event_tx = ctx.connection_event_tx.clone();
                let peer = ctx.peer;
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_incoming_streams::<C>(
                            peer,
                            (stream_tx, stream_rx),
                            connection_event_tx
                        ).await
                    {
                        tracing::error!(
                            "failed to handle incoming bi-stream with peer with index {peer}: {e:?}"
                        );
                    }
                });
            }
            accept_result = connection.accept_uni_stream() => {
                let stream_rx = accept_result?;
                let connection_event_tx = ctx.connection_event_tx.clone();
                let peer = ctx.peer;
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_incoming_uni_stream::<C>(
                            peer,
                            stream_rx,
                            connection_event_tx
                        ).await
                    {
                        tracing::error!(
                            "failed to handle incoming uni-stream from peer with index {peer}: {e:?}"
                        );
                    }
                });
            }
            driver_request = ctx.service_request_rx.recv() => {
                match driver_request {
                    Some(DriverRequest::Message(message)) => {
                        tracing::trace!("handling a broadcast message request");
                        // We need to create a new stream on the connection.
                        let connection = ctx.connection.clone();
                        let peer = ctx.peer;
                        tokio::spawn(async move{
                            if let Err(e) = create_uni_stream(connection, message).await {
                                tracing::error!(
                                    "failed to send message to peer with index {peer}: {e:?}"
                                );
                            }
                        });
                    },
                    Some(DriverRequest::NewChannel { service, respond }) => {
                        tracing::trace!("received a stream request");
                        // We need to create a new stream on the connection.
                        let connection = ctx.connection.clone();
                        let peer = ctx.peer;
                        tokio::spawn(async move {
                            if let Err(e) = create_stream(connection, service, respond).await {
                                // This may happen if `Requester::request` drops the future
                                // because of a timeout for example. In this case, it's not
                                // an issue with the connection.
                                tracing::error!(
                                    "failed to process channel request with peer with index {peer}: {e:?}"
                                );
                            }
                        });
                    }
                    None => {
                        tracing::trace!(
                            "channel was dropped: closing the connection with peer {}",
                            ctx.peer
                        );
                        ctx.connection.close(0u8, b"close from disconnect");
                        break
                    },
                }
            }
        }
    }

    Ok(())
}

async fn handle_incoming_uni_stream<C: ConnectionInterface>(
    peer: NodeIndex,
    stream_rx: C::RecvStream,
    connection_event_tx: Sender<ConnectionEvent>,
) -> Result<()> {
    let mut stream = FramedRead::new(stream_rx, LengthDelimitedCodec::new());
    while let Some(message) = stream.next().await {
        let message = Message::try_from(message?)?;
        connection_event_tx
            .send(ConnectionEvent::Broadcast { peer, message })
            .await
            .map_err(|_| anyhow::anyhow!("failed to send incoming network event"))?;
    }
    Ok(())
}

async fn handle_incoming_streams<C: ConnectionInterface>(
    peer: NodeIndex,
    (stream_tx, mut stream_rx): (C::SendStream, C::RecvStream),
    connection_event_tx: Sender<ConnectionEvent>,
) -> Result<()> {
    // The peer opened a stream.
    // The first byte identifies the service.
    let mut buf = [0u8; 1];
    stream_rx.read_exact(&mut buf).await?;
    let service_scope = ServiceScope::try_from(buf[0])?;
    connection_event_tx
        .send(ConnectionEvent::Stream {
            peer,
            service_scope,
            stream: Channel::new::<C>(stream_tx, stream_rx),
        })
        .await
        .map_err(|_| anyhow::anyhow!("failed to send incoming network event"))
}

async fn create_uni_stream<C: ConnectionInterface>(
    mut connection: C,
    message: Message,
) -> Result<()> {
    let stream_tx = connection.open_uni_stream().await?;
    let mut writer = FramedWrite::new(stream_tx, LengthDelimitedCodec::new());
    writer.send(Bytes::from(message)).await.map_err(Into::into)
}

async fn create_stream<C: ConnectionInterface>(
    mut connection: C,
    service: ServiceScope,
    respond: oneshot::Sender<io::Result<Channel>>,
) -> Result<()> {
    let (mut stream_tx, stream_rx) = connection.open_stream().await?;
    // We send the service scope first so
    // that the receiver knows the service
    // that this stream belongs to.
    stream_tx.write_all(&[service as u8]).await?;
    respond
        .send(Ok(Channel::new::<C>(stream_tx, stream_rx)))
        .map_err(|_| anyhow::anyhow!("failed to send new stream to client"))
}

/// Requests for a driver worker.
pub enum DriverRequest {
    Message(Message),
    NewChannel {
        service: ServiceScope,
        respond: oneshot::Sender<io::Result<Channel>>,
    },
}
