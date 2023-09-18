use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Sink, Stream};

#[async_trait]
pub trait Transport {
    type Sender: Sink<Bytes, Error = std::io::Error> + Unpin;
    type Receiver: Stream<Item = Bytes> + Unpin;

    fn init() -> Self;

    async fn connect(&self) -> Result<(Self::Sender, Self::Receiver)>;
}
