use std::{marker::PhantomData, sync::Arc, time::Duration};

use affair::Worker;
use async_trait::async_trait;
use draco_interfaces::{
    application::{
        ApplicationInterface, ExecutionEngineSocket, QuerySocket, SyncQueryRunnerInterface,
    },
    blockstore::BlockStoreInterface,
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    fs::FileSystemInterface,
    indexer::IndexerInterface,
    reputation::ReputationAggregatorInterface,
    signer::{SignerInterface, SubmitTxSocket},
    types::{NodeInfo, UpdateMethod},
    Blake3Hash, Blake3Tree, CompressionAlgoSet, ContentChunk, IncrementalPutInterface,
    MempoolSocket, ReputationQueryInteface, ReputationReporterInterface, SdkInterface, Weight,
};
use fleek_crypto::{
    ClientPublicKey, NodeNetworkingPublicKey, NodeNetworkingSecretKey, NodePublicKey,
    NodeSecretKey, NodeSignature,
};
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Clone)]
pub struct Signer {}

impl Worker for Signer {
    type Request = UpdateMethod;
    type Response = u64;

    fn handle(&mut self, _req: Self::Request) -> Self::Response {
        todo!()
    }
}

#[async_trait]
impl WithStartAndShutdown for Signer {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl SignerInterface for Signer {
    /// Initialize the signature service.
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    /// Provide the signer service with the mempool socket after initialization, this function
    /// should only be called once.
    fn provide_mempool(&mut self, _mempool: MempoolSocket) {
        todo!()
    }

    /// Returns the `BLS` public key of the current node.
    fn get_bls_pk(&self) -> NodePublicKey {
        todo!()
    }

    /// Returns the `Ed25519` (network) public key of the current node.
    fn get_ed25519_pk(&self) -> NodeNetworkingPublicKey {
        todo!()
    }

    /// Returns the loaded secret key material.
    ///
    /// # Safety
    ///
    /// Just like any other function which deals with secret material this function should
    /// be used with the greatest caution.
    fn get_sk(&self) -> (NodeNetworkingSecretKey, NodeSecretKey) {
        todo!()
    }

    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    ///
    /// # Panics
    ///
    /// This function can panic if there has not been a prior call to `provide_mempool`.
    fn get_socket(&self) -> SubmitTxSocket {
        todo!()
    }

    /// Sign the provided raw digest and return a signature.
    ///
    /// # Safety
    ///
    /// This function is unsafe to use without proper reasoning, which is trivial since
    /// this function is responsible for signing arbitrary messages from other parts of
    /// the system.
    fn sign_raw_digest(&self, _digest: &[u8; 32]) -> NodeSignature {
        todo!()
    }
}

impl ConfigConsumer for Signer {
    const KEY: &'static str = "signer";

    type Config = Config;
}

#[derive(Clone)]
pub struct Indexer {}

#[async_trait]
impl WithStartAndShutdown for Indexer {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl IndexerInterface for Indexer {
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    /// Publish to everyone that we have cached a content with the given `cid` successfully.
    // TODO: Put the service that caused this cid to be cached as a param here.
    fn publish(&self, _cid: &Blake3Hash) {
        todo!()
    }

    /// Returns the list of top nodes that should have a content cached.
    fn get_nodes_for_cid<Q: ReputationQueryInteface>(&self, _reputation: &Q) -> Vec<u8> {
        todo!()
    }
}

impl ConfigConsumer for Indexer {
    const KEY: &'static str = "indexer";

    type Config = Config;
}

#[derive(Clone)]
pub struct BlockStore {}

#[async_trait]
impl WithStartAndShutdown for BlockStore {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

pub struct MyPut;

#[async_trait]
impl IncrementalPutInterface for MyPut {
    fn feed_proof(&mut self, _proof: &[u8]) -> Result<(), draco_interfaces::PutFeedProofError> {
        todo!()
    }

    fn write(
        &mut self,
        _content: &[u8],
        _compression: draco_interfaces::CompressionAlgorithm,
    ) -> Result<(), draco_interfaces::PutWriteError> {
        todo!()
    }

    fn is_finished(&self) -> bool {
        todo!()
    }

    async fn finalize(
        self,
    ) -> Result<draco_interfaces::Blake3Hash, draco_interfaces::PutFinalizeError> {
        todo!()
    }
}

#[async_trait]
impl BlockStoreInterface for BlockStore {
    /// The block store has the ability to use a smart pointer to avoid duplicating
    /// the same content multiple times in memory, this can be used for when multiple
    /// services want access to the same buffer of data.
    type SharedPointer<T: ?Sized + Send + Sync> = Arc<T>;

    /// The incremental putter which can be used to write a file to block store.
    type Put = MyPut;

    /// Create a new block store from the given configuration values.
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    /// Returns the Blake3 tree associated with the given CID. Returns [`None`] if the content
    /// is not present in our block store.
    async fn get_tree(&self, _cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>> {
        todo!()
    }

    /// Returns the content associated with the given hash and block number, the compression
    /// set determines which compression modes we care about.
    ///
    /// The strongest compression should be preferred. The cache logic should take note of
    /// the number of requests for a `CID` + the supported compression so that it can optimize
    /// storage by storing the compressed version.
    ///
    /// If the content is requested with an empty compression set, the decompressed content is
    /// returned.
    async fn get(
        &self,
        _block_counter: u32,
        _block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>> {
        todo!()
    }

    /// Create a putter that can be used to write a content into the block store.
    fn put(&self, _cid: Option<Blake3Hash>) -> Self::Put {
        todo!()
    }
}

impl ConfigConsumer for BlockStore {
    const KEY: &'static str = "blockstore";

    type Config = Config;
}

#[derive(Clone)]
pub struct ReputationAggregator {}

#[async_trait]
impl ReputationAggregatorInterface for ReputationAggregator {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MyReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MyReputationQuery;

    /// Create a new reputation
    async fn init(_config: Self::Config, _submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        todo!()
    }

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter {
        todo!()
    }

    fn submit_aggregation(&self) {
        todo!()
    }
}

impl ConfigConsumer for ReputationAggregator {
    const KEY: &'static str = "reputation";

    type Config = Config;
}

#[derive(Clone)]
pub struct MyReputationQuery {}

impl ReputationQueryInteface for MyReputationQuery {
    /// The application layer's synchronize query runner.
    type SyncQuery = QueryRunner;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, _peer: &NodePublicKey) -> Option<u128> {
        todo!()
    }
}

#[derive(Clone)]
pub struct MyReputationReporter {}

impl ReputationReporterInterface for MyReputationReporter {
    /// Report a satisfactory (happy) interaction with the given peer.
    fn report_sat(&self, _peer: &NodePublicKey, _weight: Weight) {
        todo!()
    }

    /// Report a unsatisfactory (happy) interaction with the given peer.
    fn report_unsat(&self, _peer: &NodePublicKey, _weight: Weight) {
        todo!()
    }

    /// Report a latency which we witnessed from another peer.
    fn report_latency(&self, _peer: &NodePublicKey, _latency: Duration) {
        todo!()
    }

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, _peer: &NodePublicKey, _bytes: u64, _: Option<Duration>) {
        todo!()
    }

    fn report_bytes_sent(
        &self,
        _: &fleek_crypto::NodePublicKey,
        _: u64,
        _: std::option::Option<std::time::Duration>,
    ) {
        todo!()
    }

    fn report_hops(&self, _: &fleek_crypto::NodePublicKey, _: u8) {
        todo!()
    }
}

#[derive(Clone)]
pub struct FileSystem {}

#[async_trait]
impl WithStartAndShutdown for FileSystem {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl FileSystemInterface for FileSystem {
    /// The block store used for this file system.
    type BlockStore = BlockStore;

    /// The indexer used for this file system.
    type Indexer = Indexer;

    fn new(_store: &Self::BlockStore, _indexer: &Self::Indexer) -> Self {
        todo!()
    }

    /// Returns true if the given `cid` is already cached on the node.
    fn is_cached(&self, _cid: &Blake3Hash) {
        todo!()
    }

    /// Returns the tree of the provided cid.
    async fn get_tree(
        &self,
        _cid: &Blake3Hash,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<Blake3Tree>> {
        todo!()
    }

    /// Returns the requested chunk of data.
    async fn get(
        &self,
        _block_counter: u32,
        _block_hash: &Blake3Hash,
        _compression: CompressionAlgoSet,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<ContentChunk>> {
        todo!()
    }

    async fn request_download(&self, _cid: &Blake3Hash) -> bool {
        todo!()
    }
}

impl ConfigConsumer for FileSystem {
    const KEY: &'static str = "fs";

    type Config = Config;
}

use serde::{Deserialize, Serialize};

use crate::server::RawLaneConnection;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {}

pub struct Application {}

#[async_trait]
impl WithStartAndShutdown for Application {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

impl ConfigConsumer for Application {
    const KEY: &'static str = "application";

    type Config = Config;
}

#[async_trait]
impl ApplicationInterface for Application {
    /// The type for the sync query executor.
    type SyncExecutor = QueryRunner;

    /// Create a new instance of the application layer using the provided configuration.
    async fn init(_config: Self::Config) -> anyhow::Result<Self> {
        todo!()
    }

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    fn transaction_executor(&self) -> ExecutionEngineSocket {
        todo!()
    }

    /// Returns a socket that can be used to execute queries on the application layer. This
    /// socket can be passed to the *RPC* as an example.
    fn query_socket(&self) -> QuerySocket {
        todo!()
    }

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor {
        todo!()
    }
}

#[derive(Clone)]
pub struct QueryRunner {}

impl SyncQueryRunnerInterface for QueryRunner {
    /// Returns the latest balance associated with the given peer.
    fn get_balance(&self, _client: &ClientPublicKey) -> u128 {
        todo!()
    }

    /// Returns the global reputation of a node.
    fn get_reputation(&self, _node: &NodePublicKey) -> u128 {
        todo!()
    }

    /// Returns the relative score between two nodes, this score should measure how much two
    /// nodes `n1` and `n2` trust each other. Of course in real world a direct measurement
    /// between any two node might not exits, but there does exits a path from `n1` to `n2`
    /// which may cross any `node_i`, a page rank like algorithm is needed here to measure
    /// a relative score between two nodes.
    ///
    /// Existence of this data can allow future optimizations of the network topology.
    fn get_relative_score(&self, _n1: &NodePublicKey, _n2: &NodePublicKey) -> u128 {
        todo!()
    }

    /// Returns information about a single node.
    fn get_node_info(&self, _id: &NodePublicKey) -> Option<NodeInfo> {
        todo!()
    }

    /// Returns a full copy of the entire node-registry, but only contains the nodes that
    /// are still a valid node and have enough stake.
    fn get_node_registry(&self) -> Vec<NodeInfo> {
        todo!()
    }

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, _id: &NodePublicKey) -> bool {
        todo!()
    }

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u128 {
        todo!()
    }

    /// Returns the randomness that was used to start the current epoch.
    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        todo!()
    }

    /// Returns the committee members of the current epoch.
    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        todo!()
    }
}

pub struct Sdk<R, W> {
    phantom: PhantomData<(R, W)>,
    query: QueryRunner,
    rep_reporter: MyReputationReporter,
    fs: FileSystem,
    tx: SubmitTxSocket,
}

impl<R, W> Clone for Sdk<R, W> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
            query: self.query.clone(),
            rep_reporter: self.rep_reporter.clone(),
            fs: self.fs.clone(),
            tx: self.tx.clone(),
        }
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send + Sync + 'static, W: AsyncWrite + Unpin + Send + Sync + 'static>
    SdkInterface for Sdk<R, W>
{
    /// The object that is used to represent a connection in this SDK.
    type Connection = RawLaneConnection<R, W>;

    /// The type for the sync execution engine.
    type SyncQuery = QueryRunner;

    /// The reputation reporter used to report measurements about other peers.
    type ReputationReporter = MyReputationReporter;

    /// The file system of the SDK.
    type FileSystem = FileSystem;

    /// Returns a new instance of the SDK object.
    fn new(
        query: Self::SyncQuery,
        rep_reporter: Self::ReputationReporter,
        fs: Self::FileSystem,
        tx: SubmitTxSocket,
    ) -> Self {
        Self {
            phantom: PhantomData::<(R, W)>,
            query,
            rep_reporter,
            fs,
            tx,
        }
    }

    /// Returns the reputation reporter.
    fn get_reputation_reporter(&self) -> &Self::ReputationReporter {
        &self.rep_reporter
    }

    /// Returns the sync query runner.
    fn get_sync_query(&self) -> &Self::SyncQuery {
        &self.query
    }

    /// Returns the file system.
    fn get_fs(&self) -> &Self::FileSystem {
        &self.fs
    }

    /// Submit a transaction by the current node.
    fn submit_transaction(&self, _tx: UpdateMethod) {
        todo!()
    }
}

impl<R, W> ConfigConsumer for Sdk<R, W> {
    const KEY: &'static str = "sdk";

    type Config = Config;
}
