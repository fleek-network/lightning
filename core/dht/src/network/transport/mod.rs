use async_trait::async_trait;

use crate::network::message::Message;

#[async_trait]
pub trait UnreliableTransport: Clone {
    async fn send(&self, message: Message) -> std::io::Result<()>;
    async fn recv(&self) -> std::io::Result<Message>;
}
