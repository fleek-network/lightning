use std::ops::Add;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use async_channel::{bounded, Sender};
use async_trait::async_trait;
use axum::Router;
use dashmap::DashMap;
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
use lightning_schema::handshake::{HandshakeRequestFrame, TerminationReason};
use rand::RngCore;
use tokio::sync::Mutex;
use tracing::warn;
use triomphe::Arc;

use crate::config::HandshakeConfig;
use crate::http::spawn_http_server;
use crate::proxy::Proxy;
use crate::shutdown::{ShutdownNotifier, ShutdownWaiter};
use crate::transports::{
    spawn_transport_by_config,
    TransportPair,
    TransportReceiver,
    TransportSender,
};

pub struct Handshake<C: Collection> {
    status: Mutex<Option<Run<C>>>,
    config: HandshakeConfig,
}

struct Run<C: Collection> {
    ctx: Context<c![C::ServiceExecutorInterface::Provider]>,
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
        let ctx = Context::new(provider, shutdown.waiter());

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

pub struct TokenState {
    pub connection_id: u64,
    pub timeout: Option<u128>,
}

/// Shared context given to the transport listener tasks and the connection proxies.
#[derive(Clone)]
pub struct Context<P: ExecutorProviderInterface> {
    /// Service unix socket provider
    provider: P,
    pub(crate) shutdown: ShutdownWaiter,
    connection_counter: Arc<AtomicU64>,
    secondary_senders: Arc<DashMap<u64, Sender<TransportPair>>>,
    access_tokens: Arc<DashMap<[u8; 48], TokenState>>,
}

impl<P: ExecutorProviderInterface> Context<P> {
    pub fn new(provider: P, waiter: ShutdownWaiter) -> Self {
        Self {
            provider,
            shutdown: waiter,
            connection_counter: AtomicU64::new(0).into(),
            secondary_senders: DashMap::new().into(),
            access_tokens: DashMap::new().into(),
        }
    }

    pub async fn handle_new_connection<S: TransportSender, R: TransportReceiver>(
        &self,
        request: HandshakeRequestFrame,
        sender: S,
        receiver: R,
    ) where
        (S, R): Into<TransportPair>,
    {
        match request {
            // New incoming connection to a service
            HandshakeRequestFrame::Handshake {
                retry: None,
                service,
                ..
            } => {
                // TODO: Verify proof of possession
                // TODO: Send handshake response

                // Attempt to connect to the service, getting the unix socket.
                let Some(socket) = self.provider.connect(service).await else {
                    eprintln!("failed to connect");
                    sender.terminate(crate::schema::TerminationReason::InvalidService);
                    warn!("failed to connect to service {service}");
                    return;
                };

                let connection_id = self
                    .connection_counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                let (tx, rx) = bounded(1);
                self.secondary_senders.insert(connection_id, tx);

                // TODO: look into potentially more secure and audit-friendly
                //       implementations of randomness.
                let mut access_token = [0; 48];
                loop {
                    rand::thread_rng().fill_bytes(&mut access_token);
                    // active keys should never be duplicates
                    if !self.access_tokens.contains_key(&access_token) {
                        break;
                    }
                }
                self.access_tokens.insert(
                    access_token,
                    TokenState {
                        connection_id,
                        timeout: None,
                    },
                );

                Proxy::new(sender, receiver, socket, rx, access_token, self.clone()).spawn();
            },
            // Join request to an existing connection
            HandshakeRequestFrame::JoinRequest { access_token } => {
                let Some(token_state) = self.access_tokens.get(&access_token) else {
                    sender.terminate(TerminationReason::InvalidToken);
                    return;
                };

                let Some(timeout) = token_state.timeout else {
                    sender.terminate(TerminationReason::InvalidToken);
                    return;
                };

                if timeout
                    < SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("failed to get current time")
                        .as_millis()
                {
                    eprintln!("terminated");
                    sender.terminate(TerminationReason::InvalidToken);
                    return;
                }

                let Some(tx) = self.secondary_senders.get(&token_state.connection_id) else {
                    sender.terminate(TerminationReason::ServiceTerminated);
                    return;
                };

                tx.send((sender, receiver).into()).await.ok();
            },
            HandshakeRequestFrame::Handshake { retry: Some(_), .. } => {
                // TODO: Retry logic for primary connections
            },
        }
    }

    /// Initialize the token with a time to live.
    /// Returns an error if token has already been initialized.
    pub async fn handle_access_token_request(
        &self,
        access_token: &[u8; 48],
        ttl: u64,
    ) -> anyhow::Result<u64> {
        let mut state = self
            .access_tokens
            .get_mut(access_token)
            .expect("token state must exist");

        if state.timeout.is_some() {
            return Err(anyhow!("token already initialized"));
        }

        state.timeout = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("failed to get current time")
                .add(Duration::from_secs(ttl))
                .as_millis(),
        );

        Ok(ttl)
    }

    /// Extend an access token with a new ttl.
    /// Returns an error if the token has not been initialized.
    pub async fn handle_extend_access_token_request(
        &self,
        access_token: &[u8; 48],
        new_ttl: u64,
    ) -> anyhow::Result<()> {
        let mut state = self
            .access_tokens
            .get_mut(access_token)
            .expect("token state must exist");

        if state.timeout.is_none() {
            return Err(anyhow!("token has not been initialized"));
        }

        state.timeout = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("failed to get current time")
                .add(Duration::from_secs(new_ttl))
                .as_millis(),
        );

        Ok(())
    }

    pub fn cleanup_connection(&self, access_token: &[u8; 48]) {
        if let Some((_, state)) = self.access_tokens.remove(access_token) {
            self.secondary_senders.remove(&state.connection_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use lightning_blockstore::blockstore::Blockstore;
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

    #[tokio::test]
    async fn restart() -> Result<()> {
        let signer = <KeyOnlySigner as SignerInterface<TestBinding>>::init(
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let blockstore = Blockstore::init(lightning_blockstore::config::Config::default())?;
        let service_executor = ServiceExecutor::<TestBinding>::init(
            ServiceExecutorConfig::test_default(),
            &blockstore,
            affair::Socket::raw_bounded(1).0,
            Default::default(),
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
        tokio::time::sleep(Duration::from_millis(500)).await;
        // Shutdown and drop it
        handshake.shutdown().await;
        drop(handshake);

        // Start handshake again
        tokio::time::sleep(Duration::from_millis(500)).await;
        let handshake = Handshake::<TestBinding>::init(
            HandshakeConfig::default(),
            &signer,
            service_executor.get_provider(),
        )?;
        handshake.start().await;

        Ok(())
    }
}
