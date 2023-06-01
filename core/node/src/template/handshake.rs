use async_trait::async_trait;
use draco_interfaces::{
    common::WithStartAndShutdown, config::ConfigConsumer, sdk::HandlerFn, types::ServiceId,
    HandshakeInterface,
};

use super::{config::Config, sdk::Sdk};

#[derive(Clone)]
pub struct Handshake {}

#[async_trait]
impl WithStartAndShutdown for Handshake {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl HandshakeInterface for Handshake {
    type Sdk = Sdk;

    /// Initialize a new delivery acknowledgment aggregator.
    async fn init(config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    fn register_service_request_handler(
        &mut self,
        service: ServiceId,
        sdk: Self::Sdk,
        handler: HandlerFn<Self::Sdk>,
    ) {
        todo!()
    }
}

impl ConfigConsumer for Handshake {
    const KEY: &'static str = "handshake";

    type Config = Config;
}
