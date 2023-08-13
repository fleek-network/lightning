use async_trait::async_trait;

use crate::{common::WithStartAndShutdown, config::ConfigConsumer, ConnectionInterface};

#[async_trait]
pub trait HandshakeInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    // -- DYNAMIC TYPES
    // empty

    // -- BOUNDED TYPES

    /// The connection type that this handshake implementation offers.
    type Connection: ConnectionInterface;

    /// Initialize a new delivery acknowledgment aggregator.
    fn init(config: Self::Config) -> anyhow::Result<Self>;
}
