pub mod quinn;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use futures::{Sink, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::muxer::sealed::Sealed;
use crate::state::{NodeInfo, Stats};

pub type BoxedChannel =
    Box<dyn StreamAndSink<Error = io::Error, Item = io::Result<Bytes>> + Send + Sync + Unpin>;

// Todo: It might be more convenient to move this interface
// in `/interfaces` so we can pass it to `PoolInterface::init`.
/// Multiplexed-transport interface.
#[trait_variant::make(MuxerInterface: Send)]
pub trait _MuxerInterface: Clone + Send + Sync + 'static {
    // Todo: Remove this when we switch to s2n.
    // We have to keep it for now to let to
    // maintain cancel safety in `accept`.
    type Connecting: Future<Output = io::Result<Self::Connection>> + Send;
    type Connection: ConnectionInterface;
    type Config: Clone + Send;

    fn init(config: Self::Config) -> io::Result<Self>;
    async fn connect(&self, peer: NodeInfo, server_name: &str) -> io::Result<Self::Connecting>;

    fn listen_address(&self) -> io::Result<SocketAddr>;

    // The implementation must be cancel-safe.
    async fn accept(&self) -> Option<Self::Connecting>;
    async fn close(&self);
}

/// Connection over a multiplexed transport.
#[trait_variant::make(ConnectionInterface: Send)]
pub trait _ConnectionInterface: Clone + Send + 'static {
    type SendStream: AsyncWrite + Send + Sync + Unpin;
    type RecvStream: AsyncRead + Send + Sync + Unpin;

    async fn open_bi_stream(&mut self) -> io::Result<(Self::SendStream, Self::RecvStream)>;
    async fn open_uni_stream(&mut self) -> io::Result<Self::SendStream>;
    async fn accept_bi_stream(&mut self) -> io::Result<(Self::SendStream, Self::RecvStream)>;
    async fn accept_uni_stream(&mut self) -> io::Result<Self::RecvStream>;
    fn peer_identity(&self) -> Option<NodePublicKey>;
    fn remote_address(&self) -> SocketAddr;
    fn connection_id(&self) -> usize;
    fn stats(&self) -> Stats;
    fn close(&self, error_code: u8, reason: &[u8]);
}

/// A bi-directional channel intended for sending/receiving
/// messages over the network using AsyncRead/AsyncWrites handles.
// Todo: Implement custom serializer to be able to work with slices and avoid copying.
// FrameWrite/Read use an internal buffer. Ideally we would like to avoid buffering
// as much as possible, specially during writes, as data will already be buffered at
// the transport layer (muxer).
pub struct NetChannel<R, W> {
    tx: FramedWrite<W, LengthDelimitedCodec>,
    rx: FramedRead<R, LengthDelimitedCodec>,
}

impl<R, W> NetChannel<R, W>
where
    R: AsyncRead + Send + Unpin,
    W: AsyncWrite + Send + Unpin,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            tx: FramedWrite::new(writer, LengthDelimitedCodec::new()),
            rx: FramedRead::new(reader, LengthDelimitedCodec::new()),
        }
    }
}

impl<R, W> Stream for NetChannel<R, W>
where
    R: AsyncRead + Send + Unpin,
    W: AsyncWrite + Send + Unpin,
{
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Sends this is a wrapper for a Stream, the error is consumed.
        Pin::new(&mut self.rx)
            .poll_next(cx)
            .map(|bytes| bytes.map(|b| b.map(Bytes::from).map_err(Into::into)))
    }
}

impl<R, W> Sink<Bytes> for NetChannel<R, W>
where
    R: AsyncRead + Send + Unpin,
    W: AsyncWrite + Send + Unpin,
{
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx)
            .poll_ready(cx)
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx)
            .start_send(item)
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

// This trait is to deal with the only-one-non-auto-trait-allowed restriction.
pub trait StreamAndSink:
    Stream<Item = io::Result<Bytes>> + Sink<Bytes, Error = io::Error> + Sealed
{
}

impl<R, W> StreamAndSink for NetChannel<R, W>
where
    R: AsyncRead + Send + Sync + Unpin,
    W: AsyncWrite + Send + Sync + Unpin,
{
}

mod sealed {
    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::muxer::NetChannel;

    pub trait Sealed {}

    impl<R, W> Sealed for NetChannel<R, W>
    where
        R: AsyncRead + Send + Sync + Unpin,
        W: AsyncWrite + Send + Sync + Unpin,
    {
    }
}
