use fdi::BuildGraph;

use crate::infu_collection::Collection;

#[infusion::service]
pub trait HandshakeInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    // fn _init(
    //     config: ::ConfigProviderInterface,
    //     service_executor: ::ServiceExecutorInterface,
    //     keystore: ::KeystoreInterface,
    // ) {
    //     Self::init(
    //         config.get::<Self>(),
    //         keystore.clone(),
    //         service_executor.get_provider(),
    //     )
    // }

    // /// Initialize a new handshake server.
    // fn init(
    //     config: Self::Config,
    //     keystore: C::KeystoreInterface,
    //     provider: c![C::ServiceExecutorInterface::Provider],
    // ) -> anyhow::Result<Self>;
}
