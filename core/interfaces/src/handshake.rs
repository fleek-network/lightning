use infusion::c;

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::{ConfigProviderInterface, ConnectionInterface, ServiceExecutorInterface};

#[infusion::service]
pub trait HandshakeInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, executor: ::ServiceExecutorInterface) {
        Self::init(config.get::<Self>(), executor.get_connector())
    }

    /// The connection type that this handshake implementation offers.
    type Connection: ConnectionInterface;

    /// Initialize a new handshake server.
    fn init(
        config: Self::Config,
        connector: c![C::ServiceExecutorInterface::Connector],
    ) -> anyhow::Result<Self>;
}
