pub mod tcp;
pub mod webtransport;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Sink, Stream};

pub type Message = Bytes;

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    type Sender: Sink<Message, Error = std::io::Error> + Send + Unpin;
    type Receiver: Stream<Item = Message> + Send + Unpin;

    async fn connect(&self) -> Result<(Self::Sender, Self::Receiver)>;
}
