use crate::{types::ServiceId, ConfigConsumer, ConnectionInterface, WithStartAndShutdown};

pub trait ServiceExecutorInterface: WithStartAndShutdown + ConfigConsumer + Send + Sync {
    // -- DYNAMIC TYPES
    // empty

    // -- BOUNDED TYPES

    /// The connector object that this service executor provides.
    type Connector: ServiceConnectorInterface;

    /// Return the connector for this service executor.
    fn get_connector(&self) -> Self::Connector;

    /// Register a core service, a core service is any service that we ship with this binary.
    fn register_core_service(&self, service_id: ServiceId);
}

pub trait ServiceConnectorInterface: Clone {
    /// The connection specified.
    type Connection: ConnectionInterface;

    /// Handle the connection to the service specified by the name.
    fn handle(&self, service: ServiceId, connection: Self::Connection);
}
