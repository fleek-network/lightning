use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ConfigConsumer, HandshakeInterface, WithStartAndShutdown};
use serde::{Deserialize, Serialize};

use crate::worker::WorkerMode;

pub struct Handshake<C: Collection> {
    collection: PhantomData<C>,
}

#[derive(Serialize, Deserialize)]
pub struct HandshakeConfig {
    #[serde(rename = "worker")]
    workers: Vec<WorkerMode>,
}

impl Default for HandshakeConfig {
    fn default() -> Self {
        Self {
            workers: vec![
                WorkerMode::AsyncWorker,
                WorkerMode::AsyncWorker,
                WorkerMode::AsyncWorker,
                WorkerMode::AsyncWorker,
            ],
        }
    }
}

impl<C: Collection> HandshakeInterface<C> for Handshake<C> {
    fn init(_config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {
            collection: PhantomData,
        })
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
