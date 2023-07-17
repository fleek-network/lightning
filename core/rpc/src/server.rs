use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use axum::{
    routing::{get, post},
    Extension, Router,
};
use draco_interfaces::{
    common::WithStartAndShutdown, config::ConfigConsumer, MempoolSocket, RpcInterface,
    SyncQueryRunnerInterface,
};
use tokio::{sync::Notify, task};

use super::config::Config;
use crate::handlers::{rpc_handler, RpcServer};

pub struct Rpc<Q: SyncQueryRunnerInterface> {
    /// Data available to the rpc handler during a request
    data: Arc<RpcData<Q>>,
    is_running: Arc<AtomicBool>,
    pub config: Config,
    shutdown_notify: Arc<Notify>,
}

pub struct RpcData<Q: SyncQueryRunnerInterface> {
    pub query_runner: Q,
    pub mempool_socket: MempoolSocket,
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + 'static> WithStartAndShutdown for Rpc<Q> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if self.is_running() {
            return;
        }

        println!("RPC server starting up");

        let server = RpcServer::new(self.data.clone());

        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/rpc/v0", post(rpc_handler))
            .layer(Extension(server));

        self.is_running.store(true, Ordering::Relaxed);
        let http_address = SocketAddr::from(([127, 0, 0, 1], self.config.port));

        let shutdown_notify = self.shutdown_notify.clone();
        let is_running = self.is_running.clone();

        println!("listening on {http_address}");
        task::spawn(async move {
            axum::Server::bind(&http_address)
                .serve(app.into_make_service())
                .with_graceful_shutdown(shutdown_notify.notified())
                .await
                .expect("Server should not fail to start");

            println!("666");
            is_running.store(false, Ordering::Relaxed);
        });
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + Send + Sync + 'static> RpcInterface<Q> for Rpc<Q> {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: Q,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            data: Arc::new(RpcData {
                mempool_socket: mempool,
                query_runner,
            }),
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
        })
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Rpc<Q> {
    const KEY: &'static str = "rpc";

    type Config = Config;
}
