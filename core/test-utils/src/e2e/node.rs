use std::net::SocketAddr;
use std::path::PathBuf;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    EthAddress,
    NodeSecretKey,
    SecretKey,
};
use lightning_application::state::QueryRunner as ApplicationQuery;
use lightning_application::Application;
use lightning_broadcast::Broadcast;
use lightning_checkpointer::Checkpointer;
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rep_collector::MyReputationReporter;
use lightning_rpc::Rpc;
use lightning_utils::transaction::TransactionSigner;
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
    pub owner_secret_key: AccountOwnerSecretKey,
    pub app: fdi::Ref<Application<TestNodeComponents>>,
    pub app_query: ApplicationQuery,
    pub broadcast: fdi::Ref<Broadcast<TestNodeComponents>>,
    pub checkpointer: fdi::Ref<Checkpointer<TestNodeComponents>>,
    pub forwarder: fdi::Ref<MockForwarder<TestNodeComponents>>,
    pub keystore: fdi::Ref<EphemeralKeystore<TestNodeComponents>>,
    pub notifier: fdi::Ref<Notifier<TestNodeComponents>>,
    pub pool: fdi::Ref<PoolProvider<TestNodeComponents>>,
    pub rpc: fdi::Ref<Rpc<TestNodeComponents>>,
    pub reputation_reporter: fdi::Ref<MyReputationReporter>,
}

impl TestNode {
    pub async fn start(&mut self) {
        self.inner.start().await;
    }

    pub async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }

    pub fn index(&self) -> NodeIndex {
        self.app
            .sync_query()
            .pubkey_to_index(&self.keystore.get_ed25519_pk())
            .expect("failed to get node index")
    }

    pub fn get_node_info(&self) -> Option<NodeInfo> {
        self.app_query.get_node_info(&self.index(), |n| n)
    }

    pub fn get_consensus_secret_key(&self) -> ConsensusSecretKey {
        self.keystore.get_bls_sk()
    }

    pub fn get_node_secret_key(&self) -> NodeSecretKey {
        self.keystore.get_ed25519_sk()
    }

    pub fn get_owner_address(&self) -> EthAddress {
        self.owner_secret_key.to_pk().into()
    }

    pub fn get_node_signer(&self) -> TransactionSigner {
        TransactionSigner::NodeMain(self.keystore.get_ed25519_sk())
    }

    pub fn get_owner_signer(&self) -> TransactionSigner {
        TransactionSigner::AccountOwner(self.owner_secret_key.clone())
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
