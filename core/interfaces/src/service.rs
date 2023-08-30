use fn_sdk::internal::{OnConnectedArgs, OnDisconnectedArgs, OnMessageArgs};

use crate::infu_collection::Collection;
use crate::types::ServiceId;
use crate::{ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown};

/// The service executor interface is responsible for loading the services and executing
/// these services.
///
/// Currently, we are hard coding some services and there is no API on this interface to
/// load services.
#[infusion::service]
pub trait ServiceExecutorInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface) {
        Self::init(config.get::<Self>())
    }

    /// The provider which can be used to get a handle on a service during runtime.
    type Provider: ServiceHandleProviderInterface;

    /// Initialize the service executor.
    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns the service handle provider which can be used to query and get a handle to a
    /// service that is running.
    fn get_handle_provider(&self) -> Self::Provider;
}

#[infusion::blank]
pub trait ServiceHandleProviderInterface: Clone + Send + Sync + 'static {
    type Handle: ServiceHandleInterface;

    /// Returns the handle to a specific service.
    fn get_service_handle(&self, service_id: ServiceId) -> Option<Self::Handle>;
}

/// A handle to a service. The handshake can use this to send I/O related events a
/// service that is running.
#[infusion::blank]
pub trait ServiceHandleInterface: Clone + Send + Sync + 'static {
    fn connected(&self, args: OnConnectedArgs);
    fn disconnected(&self, args: OnDisconnectedArgs);
    fn message(&self, args: OnMessageArgs);

    /// Return a pending message that should be sent out from the service.
    fn poll(&self, waker: std::task::Waker) -> std::task::Poll<(u64, Vec<u8>)>;
}
