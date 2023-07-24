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

pin_project! {
    /// UDP transport for an RPC server.
    pub struct Transport<SinkMessage, Message> {
        #[pin]
        inner: UdpFramed<LengthDelimitedCodec>,
        _marker: PhantomData<(SinkMessage, Message)>,
    }
}

impl<SinkMessage, Message> Transport<SinkMessage, Message> {
    pub fn _new() -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
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
    Message: for<'a> Deserialize<'a>,
    SinkMessage: Serialize,
{
    type Item = Result<ClientMessage<Message>, TransportError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (item, address) = ready!(self.project().inner.poll_next(cx)).unwrap().unwrap();
        let mut msg: ClientMessage<Message> = match bincode::deserialize(item.as_ref()) {
            Ok(msg) => msg,
            Err(e) => return Poll::Ready(Some(Err(TransportError(e.to_string())))),
        };
        match &mut msg {
            ClientMessage::Request(request) => {
                if request.channel_id.is_some() {
                    // Todo: warn that this was set.
                } else {
                    request.channel_id = Some(address);
                }
            },
            _ => {
                return Poll::Ready(Some(Err(TransportError(
                    "invalid client message".to_string(),
                ))));
            },
        }
        Poll::Ready(Some(Ok(msg)))
    }
}

impl<SinkMessage, Message> Sink<Response<SinkMessage>> for Transport<SinkMessage, Message>
where
    SinkMessage: Serialize,
    Message: for<'a> Deserialize<'a>,
{
    type Error = TransportError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), TransportError>> {
        self.project()
            .inner
            .poll_ready(cx)
            .map_err(|e| TransportError(e.to_string()))
    }

    fn start_send(self: Pin<&mut Self>, item: Response<SinkMessage>) -> Result<(), Self::Error> {
        let address = item
            .channel_id
            .ok_or_else(|| TransportError("missing socket address".to_string()))?;
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
