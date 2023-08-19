use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::{ConfigProviderInterface, ConnectionInterface};

#[infusion::service]
pub trait HandshakeInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface) {
        Self::init(config.get::<Self>())
    }

    /// The connection type that this handshake implementation offers.
    type Connection: ConnectionInterface;

    /// Initialize a new delivery acknowledgment aggregator.
    fn init(config: Self::Config) -> anyhow::Result<Self>;
}
