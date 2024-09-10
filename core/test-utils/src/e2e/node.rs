use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use fleek_crypto::ConsensusSecretKey;
use lightning_application::Application;
use lightning_broadcast::Broadcast;
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rpc::{load_hmac_secret, Rpc, RpcClient};
use ready::tokio::TokioReadyWaiter;
use types::NodeIndex;

use super::TestNodeComponents;
use crate::keys::EphemeralKeystore;

pub struct TestNode {
    pub inner: Node<TestNodeComponents>,
    pub before_genesis_ready: TokioReadyWaiter<TestNodeBeforeGenesisReadyState>,
    pub after_genesis_ready: TokioReadyWaiter<()>,
    pub home_dir: PathBuf,

    pub app: fdi::Ref<Application<TestNodeComponents>>,
    pub keystore: fdi::Ref<EphemeralKeystore<TestNodeComponents>>,
    pub notifier: fdi::Ref<Notifier<TestNodeComponents>>,
    pub pool: fdi::Ref<PoolProvider<TestNodeComponents>>,
    pub broadcast: fdi::Ref<Broadcast<TestNodeComponents>>,
    pub rpc: fdi::Ref<Rpc<TestNodeComponents>>,
}

impl TestNode {
    pub async fn start(&mut self) {
        self.inner.start().await;
    }

    pub async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }

    pub fn get_id(&self) -> Option<NodeIndex> {
        self.app
            .sync_query()
            .pubkey_to_index(&self.keystore.get_ed25519_pk())
    }

    pub fn get_consensus_secret_key(&self) -> ConsensusSecretKey {
        self.keystore.get_bls_sk()
    }

    pub fn rpc_client(&self) -> Result<RpcClient> {
        let addr = self.rpc.listen_address().expect("rpc not ready");
        RpcClient::new_no_auth(&format!("http://{}", addr))
    }

    pub async fn rpc_admin_client(&self) -> Result<RpcClient> {
        let secret = load_hmac_secret(Some(self.home_dir.clone()))?;
        let addr = self.rpc.listen_address().expect("rpc not ready");
        RpcClient::new(&format!("http://{}/admin", addr), Some(&secret)).await
    }

    pub async fn rpc_ws_client(&self) -> Result<jsonrpsee::ws_client::WsClient> {
        let addr = self.rpc.listen_address().expect("rpc not ready");
        jsonrpsee::ws_client::WsClientBuilder::default()
            .build(&format!("ws://{}", addr))
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}

#[derive(Clone, Debug)]
pub struct TestNodeBeforeGenesisReadyState {
    pub pool_listen_address: SocketAddr,
    pub rpc_listen_address: SocketAddr,
}

impl Default for TestNodeBeforeGenesisReadyState {
    fn default() -> Self {
        Self {
            pool_listen_address: "0.0.0.0:0".parse().unwrap(),
            rpc_listen_address: "0.0.0.0:0".parse().unwrap(),
        }
    }
}
