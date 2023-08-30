use std::future::Future;

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
    type Provider: ExecutorProviderInterface;

    /// Initialize the service executor.
    fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns the service handle provider which can be used to query and get a handle to a
    /// service that is running.
    fn get_provider(&self) -> Self::Provider;
}

#[infusion::blank]
pub trait ExecutorProviderInterface: Clone + Send + Sync + 'static {
    type Handle: ServiceHandleInterface;
    type Stealer: ConnectionWorkStealer;

    /// Returns the handle to a specific service.
    fn get_service_handle(&self, service_id: ServiceId) -> Option<Self::Handle>;

    /// Return an instance of work stealer.
    fn get_work_stealer(&self) -> Self::Stealer;
}

/// A handle to a service. The handshake can use this to send I/O related events a
/// service that is running.
#[infusion::blank]
pub trait ServiceHandleInterface: Clone + Send + Sync + 'static {
    fn connected(&self, args: OnConnectedArgs);
    fn disconnected(&self, args: OnDisconnectedArgs);
    fn message(&self, args: OnMessageArgs);
}

/// A work stealer is job stealer side of worker pool that is responsible
/// for getting connection related commands coming from services and sending
/// them.
#[infusion::blank]
pub trait ConnectionWorkStealer: Clone + Send + Sync + 'static {
    type AsyncFuture: Future<Output = Option<ConnectionWork>> + Send + Sync =
        infusion::Blank<Option<ConnectionWork>>;

    /// Returns a future which return a command or `None` if there we're closing.
    fn next(&mut self) -> Self::AsyncFuture;

    /// Blocking version of the next.
    fn next_blocking(&mut self) -> Option<ConnectionWork>;
}

pub enum ConnectionWork {
    Send {
        connection_id: u64,
        payload: Vec<u8>,
    },
    Close {
        connection_id: u64,
    },
}
