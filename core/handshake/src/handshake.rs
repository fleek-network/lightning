use std::sync::atomic::AtomicU64;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_channel::{bounded, Sender};
use axum::{Extension, Router};
use axum_server::Handle;
use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use fn_sdk::header::{write_header, ConnectionHeader};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::handshake::{HandshakeRequestFrame, TerminationReason};
use rand::RngCore;
use tracing::warn;
use triomphe::Arc;

use crate::config::HandshakeConfig;
use crate::http::{self, spawn_http_server, spawn_https_server};
use crate::proxy::{Proxy, State};
use crate::transports::{
    spawn_transport_by_config,
    TransportPair,
    TransportReceiver,
    TransportSender,
};

pub struct Handshake<C: Collection> {
    status: Option<Run<C>>,
    config: HandshakeConfig,
    pk: NodePublicKey,
}

struct Run<C: Collection> {
    ctx: Context<c![C::ServiceExecutorInterface::Provider]>,
    // The axum_server Server API (TLS server) does not have a `with_graceful_shutdown`
    // similarly to axum Server. The only way to shut it down gracefully is via its Handle API.
    handle: Handle,
}

impl<C: Collection> HandshakeInterface<C> for Handshake<C> {}

impl<C: Collection> Handshake<C> {
    pub fn new(
        config: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        service_executor: &C::ServiceExecutorInterface,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) -> Self {
        let config = config.get::<Self>();
        let provider = service_executor.get_provider();
        let pk = keystore.get_ed25519_pk();
        let ctx = Context::new(provider, waiter, config.timeout);
        let handle = Handle::new();

        Self {
            status: Some(Run::<C> { ctx, handle }),
            config,
            pk,
        }
    }

    async fn start(
        fdi::Consume(mut this): fdi::Consume<Self>,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) {
        let run = this.status.take().expect("restart not implemented.");

        // Spawn transports in parallel for accepting incoming handshakes.
        let routers = this
            .config
            .transports
            .iter()
            .map(|config| {
                spawn_transport_by_config(waiter.clone(), run.ctx.clone(), config.clone())
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|res| async move { res.expect("failed to bind transport") })
            .collect::<Vec<_>>()
            .await;

        // If we have routers to use, start the http server
        if !routers.is_empty() {
            let mut router = Router::new();
            for child in routers {
                router = router.nest("", child);
            }
            let router = router
                .layer(Extension(run.ctx.clone()))
                .route_layer(http::fleek_node_response_header(this.pk));

            // Start optional HTTPS server.
            if let Some(https) = this.config.https.clone() {
                let https_router = router.clone();
                let handle = run.handle.clone();
                spawn!(
                    async move { spawn_https_server(https_router, https, handle).await },
                    "HANDSHAKE: start optional http server"
                );
            }

            // Start HTTP server.
            let waiter2 = waiter.clone();
            let http_addr = this.config.http_address;
            spawn!(
                async move { spawn_http_server(http_addr, router, waiter2).await },
                "HANDSHAKE: start http server"
            );

            // Shutdown the handle.
            waiter.wait_for_shutdown().await;
            run.handle.graceful_shutdown(Some(Duration::from_secs(2)));
        }
    }
}

impl<C: Collection> BuildGraph for Handshake<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            Self::new.with_event_handler("start", Self::start.wrap_with_spawn_named("HANDSHAKE")),
        )
    }
}

impl<C: Collection> ConfigConsumer for Handshake<C> {
    const KEY: &'static str = "handshake";
    type Config = HandshakeConfig;
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
    connections: Arc<DashMap<u64, ConnectionEntry>>,
    timeout: Duration,
}

struct ConnectionEntry {
    /// The sender half of the connection channel which can be used to notify the proxy
    /// of new connections and dials made by the user.
    connection_sender: Sender<(bool, TransportPair)>,
    /// The full access token for this connection.
    access_token: [u8; 48],
    /// The timeout for the access token.
    timeout: u128,
}

impl<P: ExecutorProviderInterface> Context<P> {
    pub fn new(provider: P, waiter: ShutdownWaiter, timeout: Duration) -> Self {
        Self {
            provider,
            shutdown: waiter,
            connection_counter: AtomicU64::new(0).into(),
            connections: DashMap::new().into(),
            timeout,
        }
    }

    pub async fn handle_new_connection<S: TransportSender, R: TransportReceiver>(
        &self,
        request: HandshakeRequestFrame,
        sender: S,
        mut receiver: R,
    ) where
        (S, R): Into<TransportPair>,
    {
        match request {
            // New incoming connection to a service
            HandshakeRequestFrame::Handshake {
                retry: None,
                service,
                pk,
                ..
            } => {
                // TODO: Verify proof of possession
                // TODO: Send handshake response

                // Attempt to connect to the service, getting the unix socket.
                let Some(mut socket) = self.provider.connect(service).await else {
                    sender.terminate(TerminationReason::InvalidService).await;
                    warn!("failed to connect to service {service}");
                    return;
                };

                let header = ConnectionHeader {
                    pk: Some(pk),
                    transport_detail: receiver.detail(),
                };

                if let Err(e) = write_header(&header, &mut socket).await {
                    sender.terminate(TerminationReason::ServiceTerminated).await;
                    warn!("failed to write connection header to service {service}: {e}");
                    return;
                }

                let connection_id = self
                    .connection_counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                let (tx, rx) = bounded(1);

                // TODO: look into potentially more secure and audit-friendly
                //       implementations of randomness.
                // For access token use the first 8 bytes as the connection id and 40 bytes of
                // random values.
                let mut access_token = [0; 48];
                rand::thread_rng().fill_bytes(&mut access_token[8..]);
                access_token[0..8].copy_from_slice(&connection_id.to_be_bytes());

                self.connections.insert(
                    connection_id,
                    ConnectionEntry {
                        connection_sender: tx,
                        access_token,
                        timeout: 0,
                    },
                );

                Proxy::new(
                    connection_id,
                    service,
                    socket,
                    rx,
                    self.clone(),
                    self.timeout,
                )
                .spawn(Some(State::OnlyPrimaryConnection(
                    (sender, receiver).into(),
                )));
            },
            // Join request to an existing connection
            HandshakeRequestFrame::JoinRequest { access_token } => {
                let connection_id = u64::from_be_bytes(*arrayref::array_ref![access_token, 0, 8]);

                let Some(connection) = self.connections.get(&connection_id) else {
                    sender.terminate(TerminationReason::InvalidToken).await;
                    return;
                };

                if connection.access_token != access_token {
                    sender.terminate(TerminationReason::InvalidToken).await;
                    return;
                }

                if connection.timeout
                    < SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("Failed to get current time")
                        .as_millis()
                {
                    sender.terminate(TerminationReason::InvalidToken).await;
                    return;
                }

                connection
                    .connection_sender
                    .send((false, (sender, receiver).into()))
                    .await
                    .ok();
            },
            HandshakeRequestFrame::Handshake {
                retry: Some(id), ..
            } => {
                let Some(connection) = self.connections.get(&id) else {
                    sender.terminate(TerminationReason::InvalidToken).await;
                    return;
                };

                connection
                    .connection_sender
                    .send((true, (sender, receiver).into()))
                    .await
                    .ok();
            },
        }
    }

    pub fn extend_access_token(&self, connection_id: u64, ttl: u64) -> ([u8; 48], u64) {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            // This should never happen.
            return ([0; 48], 0);
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get current time.")
            .as_millis();

        let new_timeout = now + (ttl * 60) as u128;
        connection.timeout = connection.timeout.max(new_timeout);
        let ttl = ((connection.timeout - now) / 60) as u64;
        (connection.access_token, ttl)
    }

    pub fn cleanup_connection(&self, connection_id: u64) {
        self.connections.remove(&connection_id);
    }
}
