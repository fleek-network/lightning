//! This file should be updated as each crate gets implemented

use std::{marker::PhantomData, time::Duration};

use affair::Worker;
use async_trait::async_trait;
use draco_application::query_runner::QueryRunner;
use draco_blockstore::memory::MemoryBlockStore;
use draco_handshake::server::RawLaneConnection;
use draco_interfaces::{
    blockstore::BlockStoreInterface,
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    fs::FileSystemInterface,
    indexer::IndexerInterface,
    reputation::ReputationAggregatorInterface,
    signer::{SignerInterface, SubmitTxSocket},
    types::UpdateMethod,
    Blake3Hash, Blake3Tree, CompressionAlgoSet, ContentChunk, MempoolSocket,
    ReputationQueryInteface, ReputationReporterInterface, SdkInterface, Weight,
};
use draco_notifier::Notifier;
use fleek_crypto::{
    NodeNetworkingPublicKey, NodeNetworkingSecretKey, NodePublicKey, NodeSecretKey, NodeSignature,
};
use serde::{Deserialize, Serialize};
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
pub struct ReputationAggregator {}

#[async_trait]
impl ReputationAggregatorInterface for ReputationAggregator {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MyReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MyReputationQuery;

    type Notifier = Notifier;

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
    fn get_reputation_of(&self, _peer: &NodePublicKey) -> Option<u8> {
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
pub struct FileSystem {
    store: MemoryBlockStore,
}

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
    type BlockStore = MemoryBlockStore;

    /// The indexer used for this file system.
    type Indexer = Indexer;

    fn new(store: &Self::BlockStore, _indexer: &Self::Indexer) -> Self {
        Self {
            store: store.clone(),
        }
    }

    /// Returns true if the given `cid` is already cached on the node.
    fn is_cached(&self, _cid: &Blake3Hash) {
        todo!()
    }

    /// Returns the tree of the provided cid.
    async fn get_tree(
        &self,
        cid: &Blake3Hash,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<Blake3Tree>> {
        self.store.get_tree(cid).await
    }

    /// Returns the requested chunk of data.
    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        compression: CompressionAlgoSet,
    ) -> Option<<Self::BlockStore as BlockStoreInterface>::SharedPointer<ContentChunk>> {
        self.store.get(block_counter, block_hash, compression).await
    }

    async fn request_download(&self, _cid: &Blake3Hash) -> bool {
        todo!()
    }
}

impl ConfigConsumer for FileSystem {
    const KEY: &'static str = "fs";

    type Config = Config;
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {}

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
