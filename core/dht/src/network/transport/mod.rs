pub mod udp;

use async_trait::async_trait;
use lightning_interfaces::types::NodeIndex;

use crate::network::message::Message;

#[async_trait]
pub trait UnreliableTransport: Clone + Send + Sync + 'static {
    async fn send(&self, message: Message, dst: NodeIndex) -> std::io::Result<()>;

    async fn recv(&self) -> std::io::Result<(NodeIndex, Message)>;
}
