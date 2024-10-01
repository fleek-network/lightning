use std::io;

use anyhow::Result;
use bytes::{Buf, Bytes};
use futures::{SinkExt, StreamExt};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{spawn, RequestHeader, ServiceScope};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::event::{Event, Message};
use crate::muxer::{ConnectionInterface, NetChannel};
use crate::provider;
use crate::provider::{Response, Status};
use crate::state::Stats;

/// Context for driving the connection.
pub struct Context<C> {
    /// The multiplexed connection.
    connection: C,
    /// The peer's index.
    peer: NodeIndex,
    /// Receive requests to perform on connection.
    service_request_rx: Receiver<Request>,
    /// Send events from this connection.
    connection_event_tx: Sender<Event>,
}

impl<C: ConnectionInterface> Context<C> {
    pub fn new(
        connection: C,
        peer: NodeIndex,
        service_request_rx: Receiver<Request>,
        connection_event_tx: Sender<Event>,
    ) -> Self {
        Self {
            connection,
            peer,
            service_request_rx,
            connection_event_tx,
        }
    }
}

pub async fn connection_loop<C: ConnectionInterface>(mut ctx: Context<C>) -> Result<()> {
    let mut connection = ctx.connection.clone();
    loop {
        tokio::select! {
            accept_result = ctx.connection.accept_bi_stream() => {
                let (stream_tx, stream_rx) = match accept_result {
                    Ok(streams) => streams,
                    Err(e) => {
                        return Err(e.into());
                    }
                };
                let connection_event_tx = ctx.connection_event_tx.clone();
                let peer = ctx.peer;
                spawn!(async move {
                    if let Err(e) =
                        handle_incoming_bi_stream::<C>(
                            peer,
                            (stream_tx, stream_rx),
                            connection_event_tx
                        ).await
                    {
                        tracing::error!(
                            "failed to handle incoming bi-stream with peer with index {peer}: {e:?}"
                        );
                    }
                }, "POOL: handle incoming bi stream");
            }
            accept_result = connection.accept_uni_stream() => {
                let stream_rx = match accept_result {
                    Ok(stream) => stream,
                    Err(e) => {
                        return Err(e.into());
                    }
                };
                let connection_event_tx = ctx.connection_event_tx.clone();
                let peer = ctx.peer;
                spawn!(async move {
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
                }, "POOL: handle incoming uni stream");
            }
            request = ctx.service_request_rx.recv() => {
                match request {
                    Some(Request::SendMessage(message)) => {
                        tracing::trace!("handling a broadcast message request");
                        // We need to create a new stream on the connection.
                        let connection = ctx.connection.clone();
                        let peer = ctx.peer;
                        spawn!(async move{
                            if let Err(e) = send_message(connection, message).await {
                                tracing::error!(
                                    "failed to send message to peer with index {peer}: {e:?}"
                                );
                            }
                        }, "POOL: send message");
                    },
                    Some(Request::SendReqResp { service, request, respond }) => {
                        tracing::trace!("handling new outgoing request");
                        // We need to create a new stream on the connection for the channel.
                        let connection = ctx.connection.clone();
                        let peer = ctx.peer;
                        spawn!(async move {
                            if let Err(e) = send_request(
                                connection,
                                service,
                                request,
                                respond
                            ).await {
                                tracing::error!(
                                    "there was an error when sending request to {peer}: {e:?}"
                                );
                            }
                        }, "POOL: send request");
                    }
                    Some(Request::Stats { respond }) => {
                        tracing::trace!("handling new stats request");
                        let _ = respond.send(ctx.connection.stats());
                    }
                    Some(Request::Close) | None => {
                        tracing::trace!(
                            "closing the connection with peer {}",
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
    connection_event_tx: Sender<Event>,
) -> Result<()> {
    let mut stream = FramedRead::new(stream_rx, LengthDelimitedCodec::new());
    while let Some(message) = stream.next().await {
        let message = Message::try_from(message?)?;
        connection_event_tx
            .send(Event::MessageReceived {
                remote: peer,
                message,
            })
            .await
            .map_err(|_| anyhow::anyhow!("failed to send incoming network event"))?;
    }
    Ok(())
}

async fn handle_incoming_bi_stream<C: ConnectionInterface>(
    peer: NodeIndex,
    (stream_tx, mut stream_rx): (C::SendStream, C::RecvStream),
    connection_event_tx: Sender<Event>,
) -> Result<()> {
    // The peer opened a stream.
    // The first byte identifies the service.
    let mut buf = [0u8; 1];
    stream_rx.read_exact(&mut buf).await?;
    let service_scope = ServiceScope::try_from(buf[0])?;

    // Read the header.
    let mut channel = Box::new(NetChannel::new(stream_rx, stream_tx));
    let bytes_header = channel
        .next()
        .await
        .ok_or(anyhow::anyhow!("missing expected header"))??;
    let header = RequestHeader {
        peer,
        bytes: bytes_header,
    };
    let request = provider::Request::new(channel);

    connection_event_tx
        .send(Event::RequestReceived {
            remote: peer,
            service_scope,
            request: (header, request),
        })
        .await
        .map_err(|_| anyhow::anyhow!("failed to send incoming network event"))
}

async fn send_message<C: ConnectionInterface>(mut connection: C, message: Message) -> Result<()> {
    let stream_tx = connection.open_uni_stream().await?;
    let mut writer = FramedWrite::new(stream_tx, LengthDelimitedCodec::new());
    writer.send(message.into()).await?;
    writer.close().await.map_err(Into::into)
}

async fn send_request<C: ConnectionInterface>(
    mut connection: C,
    service: ServiceScope,
    request: Bytes,
    respond: oneshot::Sender<io::Result<Response>>,
) -> Result<()> {
    let sending_request = async {
        let (mut stream_tx, stream_rx) = connection.open_bi_stream().await?;
        // We send the service scope first so
        // that the receiver knows the service
        // that this stream belongs to.
        stream_tx.write_all(&[service as u8]).await?;

        let mut channel = Box::new(NetChannel::new(stream_rx, stream_tx));

        // Send our request.
        channel.send(request).await?;

        // Read the response header.
        let mut header = channel.next().await.ok_or(io::ErrorKind::BrokenPipe)??;

        if header.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid encoded response status received",
            ));
        }

        let status: Status = header
            .get_u8()
            .try_into()
            .map_err(|_| io::ErrorKind::Other)?;

        Ok::<Response, io::Error>(Response::new(status, channel))
    };

    match sending_request.await {
        Ok(response) => {
            respond
                .send(Ok(response))
                .map_err(|_| anyhow::anyhow!("requester dropped the channel"))?;
        },
        Err(e) => {
            tracing::error!("unexpected error when sending a request: {e}");
            respond
                .send(Err(e))
                .map_err(|_| anyhow::anyhow!("requester dropped the channel"))?;
        },
    }

    Ok(())
}

/// Requests that will be performed on a connection.
pub enum Request {
    SendMessage(Message),
    SendReqResp {
        service: ServiceScope,
        request: Bytes,
        respond: oneshot::Sender<io::Result<Response>>,
    },
    Close,
    Stats {
        respond: oneshot::Sender<Stats>,
    },
}
