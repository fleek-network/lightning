use std::{sync::Arc, time::Duration};

use affair::Socket;
use anyhow::Result;
use async_trait::async_trait;
use draco_interfaces::{
    types::{
        CommodityServed, Epoch, EpochInfo, NodeInfo, ProtocolParams,
        ReportedReputationMeasurements, TotalServed, TransactionResponse, UpdateRequest,
    },
    ConfigConsumer, GossipInterface, GossipMessage, GossipSubscriberInterface, MempoolSocket,
    Notification, NotifierInterface, SignerInterface, SubmitTxSocket, SyncQueryRunnerInterface,
    Topic, TopologyInterface, WithStartAndShutdown,
};
use fleek_crypto::{
    ClientPublicKey, EthAddress, NodeNetworkingPublicKey, NodeNetworkingSecretKey, NodePublicKey,
    NodeSecretKey, NodeSignature,
};
use hp_float::unsigned::HpUfloat;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct MockGossip {}
pub struct MockSubscriber {}
pub struct MockSigner {}
pub struct MockTopology {}
#[derive(Clone)]
pub struct MockQueryRunner {}
pub struct MockNotifier {}

#[derive(Default, Serialize, Deserialize)]
pub struct MockConfig {}

#[async_trait]
impl WithStartAndShutdown for MockGossip {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

impl ConfigConsumer for MockGossip {
    const KEY: &'static str = "mock_gossip";

    type Config = MockConfig;
}

#[async_trait]
impl GossipInterface for MockGossip {
    type Topology = MockTopology;

    /// The notifier that allows us to refresh the connections once the epoch changes.
    type Notifier = MockNotifier;

    /// The signer that we can used to sign and submit messages.
    type Signer = MockSigner;

    /// Subscriber implementation used for listening on a topic.
    type Subscriber<T: Send + Sync + DeserializeOwned> = MockSubscriber;

    /// Initialize the gossip system with the config and the topology object..
    async fn init(
        _config: Self::Config,
        _topology: Arc<Self::Topology>,
        _signer: &Self::Signer,
    ) -> Result<Self> {
        Ok(Self {})
    }

    fn broadcast_socket(&self) -> Socket<GossipMessage, ()> {
        todo!()
    }

    fn subscribe<T: Send + Sync + DeserializeOwned>(&self, _topic: Topic) -> Self::Subscriber<T> {
        todo!()
    }
}

impl ConfigConsumer for MockSigner {
    const KEY: &'static str = "mock_signer";

    type Config = MockConfig;
}

#[async_trait]
impl SignerInterface for MockSigner {
    type SyncQuery = MockQueryRunner;

    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    fn provide_mempool(&mut self, _mempool: MempoolSocket) {}

    fn provide_query_runner(&self, _query_runner: Self::SyncQuery) {}

    fn get_bls_pk(&self) -> NodePublicKey {
        NodePublicKey([0; 96])
    }

    fn get_ed25519_pk(&self) -> NodeNetworkingPublicKey {
        NodeNetworkingPublicKey([0; 32])
    }

    fn get_sk(&self) -> (NodeNetworkingSecretKey, NodeSecretKey) {
        todo!()
    }

    fn get_socket(&self) -> SubmitTxSocket {
        todo!()
    }

    fn sign_raw_digest(&self, _digest: &[u8; 32]) -> NodeSignature {
        NodeSignature([0; 48])
    }
}

impl SyncQueryRunnerInterface for MockQueryRunner {
    fn get_account_balance(&self, _account: &EthAddress) -> u128 {
        0
    }

    fn get_client_balance(&self, _client: &ClientPublicKey) -> u128 {
        0
    }

    fn get_flk_balance(&self, _account: &EthAddress) -> HpUfloat<18> {
        HpUfloat::from(0_u64)
    }

    fn get_stables_balance(&self, _account: &EthAddress) -> HpUfloat<6> {
        HpUfloat::from(0_u64)
    }

    fn get_staked(&self, _node: &NodePublicKey) -> HpUfloat<18> {
        HpUfloat::from(0_u64)
    }

    fn get_locked(&self, _node: &NodePublicKey) -> HpUfloat<18> {
        HpUfloat::from(0_u64)
    }

    fn get_stake_locked_until(&self, _node: &NodePublicKey) -> Epoch {
        0
    }

    fn get_locked_time(&self, _node: &NodePublicKey) -> Epoch {
        0
    }

    fn get_rep_measurements(&self, _node: NodePublicKey) -> Vec<ReportedReputationMeasurements> {
        Vec::new()
    }

    fn get_reputation(&self, _node: &NodePublicKey) -> Option<u8> {
        None
    }

    fn get_relative_score(&self, _n1: &NodePublicKey, _n2: &NodePublicKey) -> u128 {
        0
    }

    fn get_node_info(&self, _id: &NodePublicKey) -> Option<NodeInfo> {
        None
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        Vec::new()
    }

    fn is_valid_node(&self, _id: &NodePublicKey) -> bool {
        true
    }

    fn get_staking_amount(&self) -> u128 {
        0
    }

    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        &[0; 32]
    }

    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        Vec::new()
    }

    fn get_epoch(&self) -> Epoch {
        0
    }

    fn get_epoch_info(&self) -> EpochInfo {
        EpochInfo {
            committee: Vec::new(),
            epoch: 0,
            epoch_end: 0,
        }
    }

    fn get_total_served(&self, _epoch: Epoch) -> TotalServed {
        TotalServed {
            served: Vec::new(),
            reward_pool: HpUfloat::from(0_u64),
        }
    }

    fn get_commodity_served(&self, _node: &NodePublicKey) -> CommodityServed {
        Vec::new()
    }

    fn get_total_supply(&self) -> HpUfloat<18> {
        HpUfloat::from(0_u64)
    }

    fn get_year_start_supply(&self) -> HpUfloat<18> {
        HpUfloat::from(0_u64)
    }

    fn get_protocol_fund_address(&self) -> EthAddress {
        EthAddress([0; 20])
    }

    /// Returns the passed in protocol parameter
    fn get_protocol_params(&self, _param: ProtocolParams) -> u128 {
        0
    }

    /// Validates the passed in transaction
    fn validate_txn(&self, _txn: UpdateRequest) -> TransactionResponse {
        todo!()
    }
}

impl NotifierInterface for MockNotifier {
    type SyncQuery = MockQueryRunner;

    fn init(_query_runner: Self::SyncQuery) -> Self {
        Self {}
    }

    fn notify_on_new_epoch(&self, _tx: mpsc::Sender<Notification>) {}

    fn notify_before_epoch_change(&self, _duration: Duration, _tx: mpsc::Sender<Notification>) {}
}

impl ConfigConsumer for MockTopology {
    const KEY: &'static str = "mock_Topology";

    type Config = MockConfig;
}

#[async_trait]
impl TopologyInterface for MockTopology {
    type SyncQuery = MockQueryRunner;

    async fn init(
        _config: Self::Config,
        _our_public_key: NodePublicKey,
        _query_runner: Self::SyncQuery,
    ) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        Arc::new(Vec::new())
    }
}

#[async_trait]
impl<T: Send + Sync + DeserializeOwned> GossipSubscriberInterface<T> for MockSubscriber {
    async fn recv(&mut self) -> Option<T> {
        None
    }
}
