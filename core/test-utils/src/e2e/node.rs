use std::any::Any;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use fdi::MultiThreadedProvider;
use fleek_crypto::{
    AccountOwnerPublicKey,
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    ConsensusSecretKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use lightning_application::state::QueryRunner;
use lightning_application::Application;
use lightning_checkpointer::Checkpointer;
use lightning_interfaces::prelude::*;
use lightning_node::ContainedNode;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rep_collector::MyReputationReporter;
use lightning_rpc::Rpc;
use lightning_signer::Signer;
use lightning_utils::transaction::TransactionSigner;
use merklize::StateRootHash;
use types::{
    Epoch,
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionRequest,
    ExecuteTransactionResponse,
    ExecuteTransactionWait,
    Genesis,
    NodeIndex,
    NodeInfo,
    NodePorts,
    UpdateMethod,
};

use super::ports::get_available_port;
use super::SyncBroadcaster;
use crate::consensus::MockForwarder;
use crate::keys::EphemeralKeystore;

#[async_trait::async_trait]
pub trait TestNetworkNode {
    fn as_any(&self) -> &dyn Any;
    fn index(&self) -> NodeIndex;
    async fn start(&mut self) -> Result<()>;
    async fn shutdown(self: Box<Self>);
    async fn apply_genesis(&self, genesis: Genesis) -> Result<()>;
    fn provider(&self) -> &MultiThreadedProvider;
    async fn get_node_ports(&self) -> Result<NodePorts>;
    async fn pool_connected_peers(&self) -> Result<Vec<NodeIndex>>;
    fn is_genesis_committee(&self) -> bool;
    fn get_owner_secret_key(&self) -> AccountOwnerSecretKey;
    fn get_owner_public_key(&self) -> AccountOwnerPublicKey;
    fn get_owner_address(&self) -> EthAddress;
    fn get_node_secret_key(&self) -> NodeSecretKey;
    fn get_node_public_key(&self) -> NodePublicKey;
    fn get_consensus_secret_key(&self) -> ConsensusSecretKey;
    fn get_consensus_public_key(&self) -> ConsensusPublicKey;
    fn app_query(&self) -> QueryRunner;
    fn emit_epoch_changed_notification(
        &self,
        epoch: Epoch,
        last_epoch_hash: [u8; 32],
        previous_state_root: StateRootHash,
        new_state_root: StateRootHash,
    );
    async fn execute_transaction_from_node(
        &self,
        method: UpdateMethod,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError>;
}

pub type BoxedTestNode = Box<dyn TestNetworkNode>;

pub struct TestFullNode<C: NodeComponents> {
    pub inner: ContainedNode<C>,
    pub home_dir: PathBuf,
    pub pool_listen_address: Option<SocketAddr>,
    pub rpc_listen_address: Option<SocketAddr>,
    pub is_genesis_committee: bool,
    pub owner_secret_key: AccountOwnerSecretKey,
}

#[async_trait::async_trait]
impl<C: NodeComponents> TestNetworkNode for TestFullNode<C> {
    async fn start(&mut self) -> Result<()> {
        // Start the node.
        tokio::time::timeout(Duration::from_secs(30), self.inner.spawn()).await???;

        // Wait for components to be ready before building genesis.
        tokio::time::timeout(Duration::from_secs(30), async move {
            // Wait for pool to be ready.
            let pool_state = self.pool().wait_for_ready().await;

            // Wait for rpc to be ready.
            let rpc_state = self.rpc().wait_for_ready().await;

            // Save the listen addresses.
            self.pool_listen_address = Some(pool_state.listen_address.unwrap());
            self.rpc_listen_address = Some(rpc_state.listen_address);
        })
        .await
        .unwrap();

        Ok(())
    }

    async fn apply_genesis(&self, genesis: Genesis) -> Result<()> {
        self.app().apply_genesis(genesis).await?;

        // Wait for components to be ready after genesis.
        tokio::time::timeout(Duration::from_secs(30), async move {
            // Wait for genesis to be applied.
            self.app_query().wait_for_genesis().await;

            // Wait for the checkpointer to be ready.
            self.checkpointer().wait_for_ready().await;
        })
        .await
        .unwrap();

        Ok(())
    }

    async fn shutdown(self: Box<Self>) {
        self.inner.shutdown().await;
    }

    async fn get_node_ports(&self) -> Result<NodePorts> {
        if self.pool_listen_address.is_none() || self.rpc_listen_address.is_none() {
            return Err(anyhow::anyhow!("node not ready"));
        }

        Ok(NodePorts {
            rpc: self.rpc_listen_address.unwrap().port(),
            pool: self.pool_listen_address.unwrap().port(),
            primary: get_available_port("127.0.0.1"),
            worker: get_available_port("127.0.0.1"),
            mempool: get_available_port("127.0.0.1"),
            ..Default::default()
        })
    }

    async fn pool_connected_peers(&self) -> Result<Vec<NodeIndex>> {
        self.pool().connected_peers().await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn index(&self) -> NodeIndex {
        self.app_query()
            .pubkey_to_index(&self.get_node_public_key())
            .expect("failed to get node index")
    }

    fn provider(&self) -> &MultiThreadedProvider {
        self.inner.provider()
    }

    fn is_genesis_committee(&self) -> bool {
        self.is_genesis_committee
    }

    fn get_owner_secret_key(&self) -> AccountOwnerSecretKey {
        self.owner_secret_key.clone()
    }

    fn get_owner_public_key(&self) -> AccountOwnerPublicKey {
        self.owner_secret_key.to_pk()
    }

    fn get_owner_address(&self) -> EthAddress {
        self.get_owner_public_key().into()
    }

    fn get_node_secret_key(&self) -> NodeSecretKey {
        self.keystore().get_ed25519_sk()
    }

    fn get_node_public_key(&self) -> NodePublicKey {
        self.keystore().get_ed25519_pk()
    }

    fn get_consensus_secret_key(&self) -> ConsensusSecretKey {
        self.keystore().get_bls_sk()
    }

    fn get_consensus_public_key(&self) -> ConsensusPublicKey {
        self.keystore().get_bls_pk()
    }

    fn app_query(&self) -> QueryRunner {
        self.app_query()
    }

    fn emit_epoch_changed_notification(
        &self,
        epoch: Epoch,
        last_epoch_hash: [u8; 32],
        previous_state_root: StateRootHash,
        new_state_root: StateRootHash,
    ) {
        self.provider()
            .get::<Notifier<C>>()
            .get_emitter()
            .epoch_changed(epoch, last_epoch_hash, previous_state_root, new_state_root);
    }

    async fn execute_transaction_from_node(
        &self,
        method: UpdateMethod,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let resp = self
            .signer()
            .get_socket()
            .run(ExecuteTransactionRequest {
                method,
                options: Some(options.unwrap_or(ExecuteTransactionOptions {
                    wait: ExecuteTransactionWait::Receipt,
                    timeout: Some(Duration::from_secs(10)),
                    ..Default::default()
                })),
            })
            .await??;

        Ok(resp)
    }
}

impl<C: NodeComponents> TestFullNode<C> {
    pub fn app(&self) -> fdi::Ref<Application<C>> {
        self.provider().get::<Application<C>>()
    }

    pub fn app_query(&self) -> QueryRunner {
        self.app().sync_query()
    }

    pub fn broadcast(&self) -> fdi::Ref<SyncBroadcaster<C>> {
        self.provider().get::<SyncBroadcaster<C>>()
    }

    pub fn keystore(&self) -> fdi::Ref<EphemeralKeystore<C>> {
        self.provider().get::<EphemeralKeystore<C>>()
    }

    pub fn rpc(&self) -> fdi::Ref<Rpc<C>> {
        self.provider().get::<Rpc<C>>()
    }

    pub fn notifier(&self) -> fdi::Ref<Notifier<C>> {
        self.provider().get::<Notifier<C>>()
    }

    pub fn forwarder(&self) -> fdi::Ref<MockForwarder<C>> {
        self.provider().get::<MockForwarder<C>>()
    }

    pub fn signer(&self) -> fdi::Ref<Signer<C>> {
        self.provider().get::<Signer<C>>()
    }

    pub fn checkpointer(&self) -> fdi::Ref<Checkpointer<C>> {
        self.provider().get::<Checkpointer<C>>()
    }

    pub fn pool(&self) -> fdi::Ref<PoolProvider<C>> {
        self.provider().get::<PoolProvider<C>>()
    }

    pub fn reputation_reporter(&self) -> fdi::Ref<MyReputationReporter> {
        self.provider().get::<MyReputationReporter>()
    }

    pub fn get_node_info(&self) -> Option<NodeInfo> {
        self.app_query().get_node_info(&self.index(), |n| n)
    }

    pub fn get_consensus_secret_key(&self) -> ConsensusSecretKey {
        self.keystore().get_bls_sk()
    }

    pub fn get_node_secret_key(&self) -> NodeSecretKey {
        self.keystore().get_ed25519_sk()
    }

    pub fn get_node_public_key(&self) -> NodePublicKey {
        self.keystore().get_ed25519_pk()
    }

    pub fn get_owner_address(&self) -> EthAddress {
        self.owner_secret_key.to_pk().into()
    }

    pub fn get_node_signer(&self) -> TransactionSigner {
        TransactionSigner::NodeMain(self.keystore().get_ed25519_sk())
    }

    pub fn get_owner_signer(&self) -> TransactionSigner {
        TransactionSigner::AccountOwner(self.owner_secret_key.clone())
    }
}

pub trait DowncastToTestFullNode {
    fn downcast<C: NodeComponents>(&self) -> &TestFullNode<C>;
}

impl<T: TestNetworkNode> DowncastToTestFullNode for T {
    fn downcast<C: NodeComponents>(&self) -> &TestFullNode<C> {
        self.as_any().downcast_ref::<TestFullNode<C>>().unwrap()
    }
}

impl DowncastToTestFullNode for Box<dyn TestNetworkNode> {
    fn downcast<C: NodeComponents>(&self) -> &TestFullNode<C> {
        self.as_any().downcast_ref::<TestFullNode<C>>().unwrap()
    }
}
