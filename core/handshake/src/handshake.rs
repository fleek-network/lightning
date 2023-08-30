use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ConfigConsumer, HandshakeInterface, WithStartAndShutdown};
use serde::{Deserialize, Serialize};

use crate::transports::Transport;

pub struct Handshake<C: Collection> {
    collection: C,
}

#[derive(Default, Serialize, Deserialize)]
pub struct HandshakeConfig {}

impl<C: Collection> HandshakeInterface<C> for Handshake<C> {
    #[doc = " Initialize a new handshake server."]
    fn init(config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }
}

impl<C: Collection> ConfigConsumer for Handshake<C> {
    const KEY: &'static str = "handshake";
    type Config = HandshakeConfig;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Handshake<C> {
    fn is_running(&self) -> bool {
        true
    }

    async fn start(&self) {}

    async fn shutdown(&self) {}
}
