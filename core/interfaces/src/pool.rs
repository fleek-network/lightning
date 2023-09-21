use netkit::endpoint::{Event, Request, ServiceScope};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::infu_collection::{c, Collection};
use crate::signer::SignerInterface;
use crate::{ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown};

/// Defines the connection pool.
#[infusion::service]
pub trait PoolInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Send + Sync + Sized
{
    fn _init(config: ::ConfigProviderInterface, signer: ::SignerInterface) {
        Self::init(config.get::<Self>(), signer)
    }

    fn init(config: Self::Config, signer: &c!(C::SignerInterface)) -> anyhow::Result<Self>;

    /// Returns an event sender and the service scope.
    /// This method should be called by each service that needs to use connection pool.
    /// This method should be called before starting the pool.
    ///
    /// # Panics
    ///
    /// This method will panic if it is called after starting the pool.
    fn network_event_receiver(&self) -> (ServiceScope, Receiver<Event>);

    /// Returns a request sender for the connection pool.
    /// This method should be called before starting the pool.
    ///
    /// # Panics
    ///
    /// This method will panic if it is called after starting the pool.
    fn request_sender(&self) -> Sender<Request>;
}
