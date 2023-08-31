use infusion::c;

use crate::common::WithStartAndShutdown;
use crate::config::ConfigConsumer;
use crate::infu_collection::Collection;
use crate::{ConfigProviderInterface, ServiceExecutorInterface, SignerInterface};

#[infusion::service]
pub trait HandshakeInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        service_executor: ::ServiceExecutorInterface,
        signer: ::SignerInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            signer,
            service_executor.get_provider(),
        )
    }

    /// Initialize a new handshake server.
    fn init(
        config: Self::Config,
        signer: &C::SignerInterface,
        provider: c![C::ServiceExecutorInterface::Provider],
    ) -> anyhow::Result<Self>;
}
