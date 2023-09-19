mod webtransport;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Sink, Stream};

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    type Sender: Sink<Bytes, Error = std::io::Error> + Send + Unpin;
    type Receiver: Stream<Item = Bytes> + Send + Unpin;

    async fn connect(&self) -> Result<(Self::Sender, Self::Receiver)>;
}
