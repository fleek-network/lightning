use std::{
    marker::PhantomData,
    net::UdpSocket,
    pin::Pin,
    task::{ready, Context, Poll},
};

use bytes::Bytes;
use futures::{Sink, Stream};
use pin_project_lite::pin_project;
use serde::{Deserialize, Serialize};
use tarpc::{ClientMessage, Response};
use tokio_util::{codec::LengthDelimitedCodec, udp::UdpFramed};

use crate::rpc::TransportError;

// Todo: Maybe we can update dependency to remove these two client and server transport.
pin_project! {
    /// UDP transport for an RPC client.
    pub struct Transport<SinkMessage, Message> {
        #[pin]
        inner: UdpFramed<LengthDelimitedCodec>,
        _marker: PhantomData<(SinkMessage, Message)>,
    }
}

impl<SinkMessage, Message> Transport<SinkMessage, Message> {
    pub fn _new(socket: UdpSocket) -> Self {
        Self {
            inner: UdpFramed::new(
                tokio::net::UdpSocket::from_std(socket).unwrap(),
                LengthDelimitedCodec::new(),
            ),
            _marker: PhantomData,
        }
    }
}

impl<SinkMessage, Message> Stream for Transport<SinkMessage, Message>
where
    Message: Serialize,
    SinkMessage: for<'a> Deserialize<'a>,
{
    type Item = Result<Response<SinkMessage>, TransportError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (item, _) = ready!(self.project().inner.poll_next(cx)).unwrap().unwrap();
        let msg: Response<SinkMessage> = match bincode::deserialize(item.as_ref()) {
            Ok(msg) => msg,
            Err(e) => return Poll::Ready(Some(Err(TransportError(e.to_string())))),
        };
        // Todo: Should we add address to response?
        Poll::Ready(Some(Ok(msg)))
    }
}

impl<SinkMessage, Message> Sink<ClientMessage<Message>> for Transport<SinkMessage, Message>
where
    Message: Serialize,
    SinkMessage: for<'a> Deserialize<'a>,
{
    type Error = TransportError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), TransportError>> {
        self.project()
            .inner
            .poll_ready(cx)
            .map_err(|e| TransportError(e.to_string()))
    }

    fn start_send(self: Pin<&mut Self>, item: ClientMessage<Message>) -> Result<(), Self::Error> {
        let channel_id = match &item {
            ClientMessage::Request(request) => request.channel_id,
            _ => {
                // Todo: Add chanel ID to Cancel message.
                return Err(TransportError("invalid client message".to_string()));
            },
        };
        let address =
            channel_id.ok_or_else(|| TransportError("missing socket address".to_string()))?;
        let bytes = bincode::serialize(&item).map_err(|e| TransportError(e.to_string()))?;
        self.project()
            .inner
            .start_send((Bytes::from(bytes), address))
            .map_err(|e| TransportError(e.to_string()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .poll_flush(cx)
            .map_err(|e| TransportError(e.to_string()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .poll_close(cx)
            .map_err(|e| TransportError(e.to_string()))
    }
}
