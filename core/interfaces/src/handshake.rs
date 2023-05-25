use async_trait::async_trait;

use crate::{
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    sdk::{HandlerFn, SdkInterface},
    types::ServiceId,
};

#[async_trait]
pub trait HandshakeInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    type Sdk: SdkInterface;

    /// Initialize a new delivery acknowledgment aggregator.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    fn register_service_request_handler(
        &mut self,
        service: ServiceId,
        sdk: Self::Sdk,
        handler: HandlerFn<Self::Sdk>,
    );
}
