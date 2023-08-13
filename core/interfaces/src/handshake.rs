use async_trait::async_trait;
use infusion::infu;

use crate::{
    common::WithStartAndShutdown, config::ConfigConsumer, infu_collection::Collection,
    ConnectionInterface,
};

#[async_trait]
pub trait HandshakeInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    infu!(HandshakeInterface @ Input);

    /// The connection type that this handshake implementation offers.
    type Connection: ConnectionInterface;

    /// Initialize a new delivery acknowledgment aggregator.
    fn init(config: Self::Config) -> anyhow::Result<Self>;
}
