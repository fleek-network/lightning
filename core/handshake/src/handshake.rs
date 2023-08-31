use async_trait::async_trait;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ConfigConsumer,
    HandshakeInterface,
    ServiceExecutorInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::shutdown::ShutdownNotifier;
use crate::state::StateRef;
use crate::transport_driver::{attach_transport_by_config, TransportConfig};
use crate::worker::{attach_worker, WorkerMode};

pub struct Handshake<C: Collection> {
    status: Mutex<Option<Run<C>>>,
    config: HandshakeConfig,
}

struct Run<C: Collection> {
    shutdown: ShutdownNotifier,
    state: StateRef<c![C::ServiceExecutorInterface::Provider]>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HandshakeConfig {
    #[serde(rename = "worker")]
    workers: Vec<WorkerMode>,
    #[serde(rename = "transport")]
    transports: Vec<TransportConfig>,
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
            transports: vec![
                // TODO
            ],
        }
    }
}

impl<C: Collection> HandshakeInterface<C> for Handshake<C> {
    fn init(
        config: Self::Config,
        signer: &C::SignerInterface,
        provider: c![C::ServiceExecutorInterface::Provider],
    ) -> anyhow::Result<Self> {
        let shutdown = ShutdownNotifier::default();
        let (_, sk) = signer.get_sk();
        let state = StateRef::new(shutdown.waiter(), sk, provider);

        Ok(Self {
            status: Mutex::new(Some(Run { shutdown, state })),
            config,
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
        self.status.blocking_lock().is_some()
    }

    async fn start(&self) {
        let guard = self.status.lock().await;
        let run = guard.as_ref().expect("restart not implemented.");

        for mode in &self.config.workers {
            attach_worker(run.state.clone(), *mode);
        }

        for config in &self.config.transports {
            attach_transport_by_config(run.state.clone(), config.clone())
                .await
                .expect("Faild to setup transport");
        }
    }

    async fn shutdown(&self) {
        let state = self.status.lock().await.take().expect("already shutdown.");
        state.shutdown.shutdown()
    }
}
