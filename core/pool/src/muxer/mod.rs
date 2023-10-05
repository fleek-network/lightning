pub mod quinn;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use fleek_crypto::NodePublicKey;
use futures::future::Either;
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_util::sync::PollSender;

use crate::endpoint::NodeAddress;

// Todo: It might be more convenient to move this interface
// in `/interfaces` so we can pass it to `PoolInterface::init`.
/// Multiplexed-transport interface.
#[async_trait]
pub trait MuxerInterface: Clone + Send + Sync + 'static {
    type Connecting: Future<Output = io::Result<Self::Connection>> + Send;
    type Connection: ConnectionInterface;
    type Config: Clone;

    fn init(config: Self::Config) -> io::Result<Self>;
    async fn connect(&self, peer: NodeAddress, server_name: &str) -> io::Result<Self::Connecting>;
    async fn accept(&self) -> Option<Self::Connecting>;
}

/// Connection over a multiplexed transport.
#[async_trait]
pub trait ConnectionInterface: Clone + Send + 'static {
    type SendStream: AsyncWrite + Send + Unpin;
    type RecvStream: AsyncRead + Send + Unpin;

    async fn open_stream(&mut self) -> io::Result<(Self::SendStream, Self::RecvStream)>;
    async fn open_uni_stream(&mut self) -> io::Result<Self::SendStream>;
    async fn accept_stream(&mut self) -> io::Result<(Self::SendStream, Self::RecvStream)>;
    async fn accept_uni_stream(&mut self) -> io::Result<Self::RecvStream>;
    fn peer_identity(&self) -> Option<NodePublicKey>;
    fn remote_address(&self) -> SocketAddr;
    fn connection_id(&self) -> usize;
    fn close(&self, error_code: u8, reason: &[u8]);
}

/// A bi-directional channel for sending messages.
///
/// Implements `Stream` and `Sink`.
// This helps us avoid adding generics for abstracting the
// (AsyncWrite, AsyncWrite) pair that is allocated in a driver
// and returned to a user.
pub struct Channel {
    tx: PollSender<Result<Bytes, io::Error>>,
    rx: ReceiverStream<BytesMut>,
}

impl Channel {
    pub fn new<C: ConnectionInterface>(send: C::SendStream, recv: C::RecvStream) -> Self {
        let mut framed_write = FramedWrite::new(send, LengthDelimitedCodec::new());
        let mut framed_read = FramedRead::new(recv, LengthDelimitedCodec::new());

        let (bi_stream_tx, bi_stream_rx) = mpsc::channel(2048);
        let (worker_tx, worker_rx) = mpsc::channel(2048);

        tokio::spawn(async move {
            // We can ignore this error because it's the value that was supposed to have been sent.
            let mut sink =
                PollSender::new(bi_stream_tx).sink_map_err(|_| io::ErrorKind::BrokenPipe.into());
            let stream = ReceiverStream::new(worker_rx);

            match futures::future::select(
                // We receive data from the network on this `Stream`/`FrameRead`.
                // The sink then sends all the data using a Sender,
                // we receive the data on a `ReceiverStream`.
                sink.send_all(&mut framed_read),
                // Using our PollSender, we send data to this `Stream`,
                // this stream sends the data on this `Sink`/`FrameWriter`
                // which sends it on the network.
                stream.forward(&mut framed_write),
            )
            .await
            {
                Either::Left((value1, fut)) => {
                    if let Err(e) = value1 {
                        tracing::error!("unexpected with error when sending a message: {e:?}");
                    }

                    if let Err(e) = fut.await {
                        tracing::error!("unexpected with error when receiving a message: {e:?}");
                    }
                },
                Either::Right((value2, fut)) => {
                    if let Err(e) = value2 {
                        tracing::error!("unexpected with error when receiving a message: {e:?}");
                    }

                    if let Err(e) = fut.await {
                        tracing::error!("unexpected with error when sending a message: {e:?}");
                    }
                },
            }
        });

        Self {
            tx: PollSender::new(worker_tx),
            rx: ReceiverStream::new(bi_stream_rx),
        }
    }
}

impl Stream for Channel {
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Sends this is a wrapper for a Stream, the error is consumed.
        Pin::new(&mut self.rx)
            .poll_next(cx)
            .map(|bytes| bytes.map(|b| Result::<_, io::Error>::Ok(Bytes::from(b))))
    }
}

impl Sink<Bytes> for Channel {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx)
            .poll_ready(cx)
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx)
            .start_send(Ok(item))
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx)
            .poll_flush(cx)
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx)
            .poll_close(cx)
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }
}
