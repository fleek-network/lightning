use infusion::{c, Blank};

use crate::infu_collection::Collection;
use crate::types::ServiceId;
use crate::{
    ConfigConsumer,
    ConfigProviderInterface,
    ConnectionInterface,
    HandshakeInterface,
    WithStartAndShutdown,
};

#[infusion::service]
pub trait ServiceExecutorInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface) {
        Self::init(config.get::<Self>())
    }

    /// The connector object that this service executor provides.
    type Connector: ServiceConnectorInterface<c![C::HandshakeInterface::Connection]> =
        Blank<c![C::HandshakeInterface::Connection]>;

    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Return the connector for this service executor.
    #[blank = Default::default()]
    fn get_connector(&self) -> Self::Connector;

    /// Register a core service, a core service is any service that we ship with this binary.
    fn register_core_service(&self, service_id: ServiceId);
}

#[infusion::blank]
pub trait ServiceConnectorInterface<Connection: ConnectionInterface>: Clone {
    /// Handle the connection to the service specified by the name.
    fn handle(&self, service: ServiceId, connection: Connection);
}
