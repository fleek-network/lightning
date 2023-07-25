use std::{collections::HashMap, marker::PhantomData, sync::Arc, time::Duration};

use affair::Socket;
use anyhow::Result;
use async_trait::async_trait;
use draco_interfaces::{
    types::{
        Epoch, EpochInfo, NodeInfo, NodeServed, ProtocolParams, ReportedReputationMeasurements,
        Service, ServiceId, TotalServed, TransactionResponse, UpdateRequest,
    },
    Blake3Hash, ConfigConsumer, GossipInterface, IndexerInterface, MempoolSocket, Notification,
    NotifierInterface, PubSub, ReputationAggregatorInterface, ReputationQueryInteface,
    ReputationReporterInterface, SignerInterface, SubmitTxSocket, SyncQueryRunnerInterface, Topic,
    TopologyInterface, Weight, WithStartAndShutdown,
};
use fleek_crypto::{
    ClientPublicKey, EthAddress, NodeNetworkingPublicKey, NodeNetworkingSecretKey, NodePublicKey,
    NodeSecretKey, NodeSignature,
};
use hp_float::unsigned::HpUfloat;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::{mpsc, Notify};

pub struct MockGossip {}
pub struct MockSubscriber {}
pub struct MockSigner {
    socket: SubmitTxSocket,
}
pub struct MockTopology {}
#[derive(Clone)]
pub struct MockQueryRunner {}
pub struct MockNotifier {}
#[derive(Clone)]
pub struct MockReputationAggregator {}
#[derive(Clone)]
pub struct MockReputationQuery {}
#[derive(Clone)]
pub struct MockReputationReporter {}
#[derive(Default, Serialize, Deserialize)]
pub struct MockConfig {}
#[derive(Clone)]
pub struct MockIndexer {}
#[derive(Clone, Serialize)]
pub struct MockPubSub<T: Serialize + DeserializeOwned + Send + Sync + Clone> {
    pub _data: PhantomData<T>,
}

#[async_trait]
impl WithStartAndShutdown for MockIndexer {
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

#[async_trait]
impl IndexerInterface for MockIndexer {
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    /// Publish to everyone that we have cached a content with the given `cid` successfully.
    // TODO: Put the service that caused this cid to be cached as a param here.
    fn publish(&self, _cid: &Blake3Hash) {}

    /// Returns the list of top nodes that should have a content cached.
    fn get_nodes_for_cid<Q: ReputationQueryInteface>(&self, _reputation: &Q) -> Vec<u8> {
        Vec::new()
    }
}

impl ConfigConsumer for MockIndexer {
    const KEY: &'static str = "indexer";

    type Config = MockConfig;
}

impl ReputationQueryInteface for MockReputationQuery {
    /// The application layer's synchronize query runner.
    type SyncQuery = MockQueryRunner;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, _peer: &NodePublicKey) -> Option<u8> {
        Some(1)
    }
}

impl ReputationReporterInterface for MockReputationReporter {
    /// Report a satisfactory (happy) interaction with the given peer.
    fn report_sat(&self, _peer: &NodePublicKey, _weight: Weight) {}

    /// Report a unsatisfactory (happy) interaction with the given peer.
    fn report_unsat(&self, _peer: &NodePublicKey, _weight: Weight) {}

    /// Report a latency which we witnessed from another peer.
    fn report_latency(&self, _peer: &NodePublicKey, _latency: Duration) {}

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, _peer: &NodePublicKey, _bytes: u64, _: Option<Duration>) {}

    fn report_bytes_sent(
        &self,
        _: &fleek_crypto::NodePublicKey,
        _: u64,
        _: std::option::Option<std::time::Duration>,
    ) {
    }

    fn report_hops(&self, _: &fleek_crypto::NodePublicKey, _: u8) {}
}

#[async_trait]
impl ReputationAggregatorInterface for MockReputationAggregator {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MockReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MockReputationQuery;

    type Notifier = MockNotifier;

    /// Create a new reputation
    async fn init(
        _config: Self::Config,
        _submit_tx: SubmitTxSocket,
        _notifier: Self::Notifier,
    ) -> anyhow::Result<Self> {
        todo!()
    }

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter {
        todo!()
    }

    /// Returns a reputation query that can be used to answer queries about the local
    /// reputation we have of another peer.
    fn get_query(&self) -> Self::ReputationQuery {
        todo!()
    }

    fn submit_aggregation(&self) {
        todo!()
    }
}

impl ConfigConsumer for MockReputationAggregator {
    const KEY: &'static str = "reputation";

    type Config = MockConfig;
}

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

    type PubSub<T: Serialize + DeserializeOwned + Send + Sync + Clone> = MockPubSub<T>;

    /// Initialize the gossip system with the config and the topology object..
    async fn init(
        _config: Self::Config,
        _topology: Arc<Self::Topology>,
        _signer: &Self::Signer,
    ) -> Result<Self> {
        Ok(Self {})
    }

    fn get_pubsub<T: Serialize + DeserializeOwned + Send + Sync + Clone>(
        &self,
        _topic: Topic,
    ) -> Self::PubSub<T> {
        MockPubSub {
            _data: PhantomData::<T>,
        }
    }
}

impl ConfigConsumer for MockSigner {
    const KEY: &'static str = "mock_signer";

    type Config = MockConfig;
}

#[async_trait]
impl<T: Serialize + DeserializeOwned + Send + Sync + Clone> PubSub<T> for MockPubSub<T> {
    /// Publish a message.
    fn send(&self, _msg: &T) {}

    /// Await the next message in the topic, should only return `None` if there are
    /// no longer any new messages coming. (indicating that the gossip instance is
    /// shutdown.)
    async fn recv(&mut self) -> Option<T> {
        None
    }
}

#[async_trait]
impl WithStartAndShutdown for MockSigner {
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

#[async_trait]
impl SignerInterface for MockSigner {
    type SyncQuery = MockQueryRunner;

    async fn init(_config: Self::Config, _query_runner: Self::SyncQuery) -> anyhow::Result<Self> {
        let (socket, _) = Socket::raw_bounded(2048);
        Ok(Self { socket })
    }

    fn provide_mempool(&mut self, _mempool: MempoolSocket) {}

    fn provide_new_block_notify(&self, _block_notify: Arc<Notify>) {}

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
        self.socket.clone()
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

    fn get_node_served(&self, _node: &NodePublicKey) -> NodeServed {
        NodeServed::default()
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

    fn get_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
        HashMap::new()
    }

    fn get_service_info(&self, _service_id: ServiceId) -> Service {
        Service {
            owner: EthAddress([0; 20]),
            commodity_type: draco_interfaces::types::CommodityTypes::Bandwidth,
            slashing: (),
        }
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
