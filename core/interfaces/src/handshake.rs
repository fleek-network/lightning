use infusion::infu;

use crate::{
    common::WithStartAndShutdown, config::ConfigConsumer, infu_collection::Collection,
    ConfigProviderInterface, ConnectionInterface,
};

#[infusion::blank]
pub trait HandshakeInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    infu!(HandshakeInterface, {
        fn init(config: ConfigProviderInterface) {
            Self::init(config.get::<Self>())
        }
    });

    /// The connection type that this handshake implementation offers.
    type Connection: ConnectionInterface;

    /// Initialize a new delivery acknowledgment aggregator.
    fn init(config: Self::Config) -> anyhow::Result<Self>;
}
