use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use axum::{
    routing::post,
    Extension, Router,
};
use draco_interfaces::{
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    types::{QueryMethod, QueryRequest, TransactionResponse},
    MempoolSocket, QuerySocket, RpcInterface, RpcMethods,
};
use fleek_crypto::{AccountOwnerPublicKey, TransactionSender::AccountOwner};

use super::config::Config;
use crate::handlers::{rpc_handler, RpcServer};

#[derive(Clone)]
pub struct Rpc {
    _mempool_address: MempoolSocket,
    query_socket: QuerySocket,
    config: Config,
    server_running: Arc<RwLock<bool>>,
}

impl Rpc {
    fn set_running(&self, status: bool) {
        if let Ok(mut server_running) = self.server_running.write() {
            *server_running = status;
        }
    }
}

#[async_trait]
impl WithStartAndShutdown for Rpc {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        *self.server_running.read().unwrap()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if !self.is_running() {
            println!("server starting up");
            let rpc = Arc::new(self.clone());
            let server = RpcServer::new(Arc::clone(&rpc));

            let app = Router::new()
                .route("/rpc/v0", post(rpc_handler))
                .layer(Extension(server.clone()));

            self.set_running(true);
            let http_address = SocketAddr::from(([127, 0, 0, 1], self.config.port));
            axum::Server::bind(&http_address)
                .serve(app.into_make_service())
                .await
                .expect("Server should not fail to start");
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.set_running(false);
        // more loggic here
        todo!()
    }
}

#[async_trait]
impl RpcInterface for Rpc {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_socket: QuerySocket,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            _mempool_address: mempool,
            query_socket,
            config,
            server_running: Arc::new(RwLock::new(false)),
        })
    }
}
#[async_trait]
impl RpcMethods for Rpc {
    /// ping method for rpc server, for clients to check if server is online
    async fn ping(&self) -> anyhow::Result<String> {
        Ok("pong".to_string())
    }

    /// this method would fetch the account balance of a particular address or account.
    async fn get_balance(&self, public_key: AccountOwnerPublicKey) -> TransactionResponse {
        let query = QueryRequest {
            sender: AccountOwner(public_key),
            query: QueryMethod::FLK { public_key },
        };
        let res = self.query_socket.run(query).await.unwrap();
        bincode::deserialize(&res).unwrap()
    }

    /// This method would return information about a specific node.
    async fn get_node_info(&self) {
        todo!()
    }

    /// This method would return global information about the network
    async fn get_network_info(&self) {
        todo!()
    }

    /// This method would allow a client to submit a transaction to the network.
    async fn submit_transaction(&self) {
        todo!()
    }

    /// This method would allow a client to retrieve information about a specific block by its
    /// height or hash.
    async fn get_block(&self) {
        todo!()
    }

    /// This endpoint would allow a client to retrieve information about a specific transaction by
    /// its hash.
    async fn get_transaction(&self) {
        todo!()
    }
}

impl ConfigConsumer for Rpc {
    const KEY: &'static str = "rpc";

    type Config = Config;
}
