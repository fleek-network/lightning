use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ConfigConsumer,
    ExecutorProviderInterface,
    HandshakeInterface,
    ServiceExecutorInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use tokio::sync::Mutex;
use triomphe::Arc;

use crate::config::HandshakeConfig;
use crate::http::spawn_http_server;
use crate::proxy::Proxy;
use crate::shutdown::{ShutdownNotifier, ShutdownWaiter};
use crate::transports::{spawn_transport_by_config, TransportReceiver, TransportSender};

/// Default connection timeout. This is the amount of time we will wait
/// to close a connection after all transports have dropped.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Handshake<C: Collection> {
    status: Mutex<Option<Run<C>>>,
    config: HandshakeConfig,
}

struct Run<C: Collection> {
    ctx: Arc<Context<c![C::ServiceExecutorInterface::Provider]>>,
    shutdown: ShutdownNotifier,
}

impl<C: Collection> HandshakeInterface<C> for Handshake<C> {
    fn init(
        config: Self::Config,
        signer: &C::SignerInterface,
        provider: c![C::ServiceExecutorInterface::Provider],
    ) -> anyhow::Result<Self> {
        let shutdown = ShutdownNotifier::default();
        let (_, _) = signer.get_sk();
        let ctx = Context::new(provider, shutdown.waiter()).into();

        Ok(Self {
            status: Mutex::new(Some(Run::<C> { ctx, shutdown })),
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

        // Spawn transport listeners to accept incoming handshakes
        let mut routers = vec![];
        for config in &self.config.transports {
            if let Some(router) =
                spawn_transport_by_config(run.shutdown.waiter(), run.ctx.clone(), config.clone())
                    .await
                    .expect("Failed to setup transport")
            {
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
    }
}

pub struct Context<P: ExecutorProviderInterface> {
    /// Service unix socket provider
    provider: P,
    pub(crate) shutdown: ShutdownWaiter,
}

impl<P: ExecutorProviderInterface> Context<P> {
    pub fn new(provider: P, waiter: ShutdownWaiter) -> Self {
        Self {
            provider,
            shutdown: waiter,
        }
    }

    pub async fn handle_new_connection<S: TransportSender, R: TransportReceiver>(
        &self,
        service_id: u32,
        tx: S,
        rx: R,
    ) {
        // Attempt to connect to the service, getting the unix socket.
        let Some(socket) = self.provider.connect(service_id).await else {
            tx.terminate(crate::schema::TerminationReason::InvalidService);
            return;
        };

        // Spawn a new proxy for this connection
        Proxy::new(tx, rx, socket, self.shutdown.clone()).spawn();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use affair::{Executor, TokioSpawn, Worker};
    use anyhow::Result;
    use lightning_blockstore::blockstore::Blockstore;
    use lightning_interfaces::types::{FetcherRequest, FetcherResponse, TransactionRequest};
    use lightning_interfaces::{partial, BlockStoreInterface};
    use lightning_service_executor::shim::{ServiceExecutor, ServiceExecutorConfig};
    use lightning_test_utils::keys::KeyOnlySigner;

    use super::*;

    partial!(TestBinding {
        HandshakeInterface = Handshake<Self>;
        ServiceExecutorInterface = ServiceExecutor<Self>;
        SignerInterface = KeyOnlySigner;
        BlockStoreInterface = Blockstore<Self>;
    });

    struct DummyWorker2;
    impl Worker for DummyWorker2 {
        type Request = FetcherRequest;
        type Response = FetcherResponse;
        fn handle(&mut self, _: Self::Request) -> Self::Response {
            todo!()
        }
    }

    #[tokio::test]
    async fn restart() -> Result<()> {
        let signer = <KeyOnlySigner as SignerInterface<TestBinding>>::init(Default::default(), Default::default()).unwrap();
        let socket2 = TokioSpawn::spawn(DummyWorker2);
        let blockstore = Blockstore::init(lightning_blockstore::config::Config::default())?;
        let service_executor = ServiceExecutor::<TestBinding>::init(
            ServiceExecutorConfig::test_default(),
            &blockstore,
            socket2,
        )?;
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
