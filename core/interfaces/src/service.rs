use infusion::infu;

use crate::{
    infu_collection::Collection, types::ServiceId, ConfigConsumer, ConfigProviderInterface,
    ConnectionInterface, WithStartAndShutdown,
};

#[infusion::blank]
pub trait ServiceExecutorInterface:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    infu!(ServiceExecutorInterface, {
        fn init(config: ConfigProviderInterface) {
            Self::init(config.get::<Self>())
        }
    });

    /// The connector object that this service executor provides.
    type Connector: ServiceConnectorInterface;

    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Return the connector for this service executor.
    fn get_connector(&self) -> Self::Connector;

    /// Register a core service, a core service is any service that we ship with this binary.
    fn register_core_service(&self, service_id: ServiceId);
}

#[infusion::blank(object = true)]
pub trait ServiceConnectorInterface: Clone {
    /// The connection specified.
    type Connection: ConnectionInterface;

    /// Handle the connection to the service specified by the name.
    fn handle(&self, service: ServiceId, connection: Self::Connection);
}
