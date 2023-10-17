use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
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
use tokio::task::JoinHandle;

use crate::http::spawn_http_server;
use crate::shutdown::ShutdownNotifier;
use crate::state::StateRef;
use crate::transport_driver::{attach_transport_by_config, TransportConfig};
use crate::worker::{attach_worker, WorkerMode};

/// Default connection timeout. This is the amount of time we will wait
/// to close a connection after all transports have dropped.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Handshake<C: Collection> {
    status: Mutex<Option<Run<C>>>,
    config: HandshakeConfig,
}

struct Run<C: Collection> {
    shutdown: ShutdownNotifier,
    state: StateRef<c![C::ServiceExecutorInterface::Provider]>,
    transports: Vec<JoinHandle<()>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HandshakeConfig {
    #[serde(rename = "worker")]
    pub workers: Vec<WorkerMode>,
    #[serde(rename = "transport")]
    pub transports: Vec<TransportConfig>,
    pub http_address: SocketAddr,
    pub max_client_connection_limit: usize,
    pub max_global_connection_limit: usize,
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
                TransportConfig::WebRTC(Default::default()),
                TransportConfig::Tcp(Default::default()),
            ],
            http_address: ([0, 0, 0, 0], 4220).into(),
            max_client_connection_limit: 254,
            max_global_connection_limit: 10_000,
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
        let state = StateRef::new(
            CONNECTION_TIMEOUT,
            shutdown.waiter(),
            sk,
            provider,
            config.max_client_connection_limit,
            config.max_global_connection_limit,
        );

        Ok(Self {
            status: Mutex::new(Some(Run {
                shutdown,
                state,
                transports: vec![],
            })),
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
        let mut guard = self.status.lock().await;
        let run = guard.as_mut().expect("restart not implemented.");

        // Attach workers
        for mode in &self.config.workers {
            attach_worker(run.state.clone(), *mode);
        }

        // Attach transports
        let mut routers = vec![];
        for config in &self.config.transports {
            let (handle, router) = attach_transport_by_config(run.state.clone(), config.clone())
                .await
                .expect("Faild to setup transport");

            run.transports.push(handle);
            if let Some(router) = router {
                routers.push(router)
            }
        }

        // If we have routers to use, start the http server
        if !routers.is_empty() {
            let mut router = Router::new();
            for child in routers {
                router = router.nest("", child);
            }
            let waiter = run.shutdown.waiter();
            let http_addr = self.config.http_address;
            tokio::spawn(async move { spawn_http_server(http_addr, router, waiter).await });
        }
    }

    async fn shutdown(&self) {
        let run = self.status.lock().await.take().expect("already shutdown.");
        run.shutdown.shutdown();

        // give time to transports and then abort.
        tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(3)).await;

            for handle in run.transports {
                handle.abort();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use affair::{Executor, TokioSpawn, Worker};
    use anyhow::Result;
    use infusion::Blank;
    use lightning_blockstore::blockstore::Blockstore;
    use lightning_interfaces::types::TransactionRequest;
    use lightning_interfaces::{partial, BlockStoreInterface};
    use lightning_service_executor::shim::{ServiceExecutor, ServiceExecutorConfig};
    use lightning_signer::Signer;
    use tokio::sync::Notify;

    use super::*;

    partial!(TestBinding {
        HandshakeInterface = Handshake<Self>;
        ServiceExecutorInterface = ServiceExecutor<Self>;
        SignerInterface = Signer<Self>;
        BlockStoreInterface = Blockstore<Self>;
    });

    struct DummyWorker;
    impl Worker for DummyWorker {
        type Request = TransactionRequest;
        type Response = ();
        fn handle(&mut self, _: Self::Request) -> Self::Response {
            todo!()
        }
    }

    #[tokio::test]
    async fn restart() -> Result<()> {
        let mut signer = Signer::init(lightning_signer::Config::test(), Blank::default())?;
        let socket = TokioSpawn::spawn(DummyWorker);
        signer.provide_mempool(socket);
        signer.provide_new_block_notify(Notify::default().into());
        let blockstore = Blockstore::init(lightning_blockstore::config::Config::default())?;
        let service_executor =
            ServiceExecutor::<TestBinding>::init(ServiceExecutorConfig::default(), &blockstore)?;
        signer.start().await;
        service_executor.start().await;

        // Startup handshake
        let handshake = Handshake::<TestBinding>::init(
            HandshakeConfig::default(),
            &signer,
            service_executor.get_provider(),
        )?;
        handshake.start().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        // Shutdown and drop it
        handshake.shutdown().await;
        drop(handshake);

        // Start handshake again
        tokio::time::sleep(Duration::from_secs(1)).await;
        let handshake = Handshake::<TestBinding>::init(
            HandshakeConfig::default(),
            &signer,
            service_executor.get_provider(),
        )?;
        handshake.start().await;

        Ok(())
    }
}
