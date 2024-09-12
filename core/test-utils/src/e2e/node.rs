use std::net::SocketAddr;
use std::path::PathBuf;

use fleek_crypto::{ConsensusSecretKey, NodeSecretKey};
use lightning_application::Application;
use lightning_broadcast::Broadcast;
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rpc::Rpc;
use ready::tokio::TokioReadyWaiter;
use types::{NodeIndex, NodeInfo};

use super::TestNodeComponents;
use crate::consensus::MockForwarder;
use crate::keys::EphemeralKeystore;

pub struct TestNode {
    pub inner: Node<TestNodeComponents>,
    pub before_genesis_ready: TokioReadyWaiter<TestNodeBeforeGenesisReadyState>,
    pub after_genesis_ready: TokioReadyWaiter<()>,
    pub home_dir: PathBuf,

    pub app: fdi::Ref<Application<TestNodeComponents>>,
    pub broadcast: fdi::Ref<Broadcast<TestNodeComponents>>,
    pub forwarder: fdi::Ref<MockForwarder<TestNodeComponents>>,
    pub keystore: fdi::Ref<EphemeralKeystore<TestNodeComponents>>,
    pub notifier: fdi::Ref<Notifier<TestNodeComponents>>,
    pub pool: fdi::Ref<PoolProvider<TestNodeComponents>>,
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

    pub fn get_node_info(&self) -> Option<NodeInfo> {
        let node_id = self.get_id()?;
        self.app.sync_query().get_node_info(&node_id, |n| n)
    }

    pub fn get_consensus_secret_key(&self) -> ConsensusSecretKey {
        self.keystore.get_bls_sk()
    }

    pub fn get_node_secret_key(&self) -> NodeSecretKey {
        self.keystore.get_ed25519_sk()
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
