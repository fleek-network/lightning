use std::{collections::HashMap, ops::Deref};

use affair::Socket;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

/// An implementer of this trait should handle providing the configurations from
/// the loaded configuration file.
pub trait ConfigProviderInterface: Send + Sync {
    /// Returns the configuration for the given object. If the key is not present
    /// in the loaded file we should return the default object.
    fn get<S: ConfigConsumer>(&self) -> S::Config;

    /// Returns the textual representation of the configuration based on all values
    /// that have been loaded so far.
    fn serialize_config(&self) -> String;
}

/// Any object that in the program that is associated a configuration value
/// in the global configuration file.
pub trait ConfigConsumer {
    /// The top-level key in the config file that should be used for this object.
    const KEY: &'static str;

    /// The type which is expected for this configuration object.
    type Config: Serialize + DeserializeOwned + Default;
}

pub enum TransactionList {
    // x
}

#[async_trait]
pub trait WithStartAndShutdown {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool;

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self);

    /// Send the shutdown signal to the system.
    async fn shutdown(&self);
}

/// A port that gives services and other sub-systems the required functionality to
/// submit messages/transactions to the consensus.
///
/// # Safety
///
/// This port is safe to freely pass around, sending transactions through this port
/// does not guarantee their execution on the application layer. You can think about
/// this as if the current node was only an external client to the network.
pub type MempoolPort = Socket<TransactionList, ()>;

/// The port that is handled by the application layer and fed by consensus (or other
/// synchronization systems in place) which executes and persists transactions that
/// are put into it.
///
/// # Safety
///
/// This port should be used with as much caution as possible, for all intend and purposes
/// this port should be sealed and preferably not accessible out side of the scope in which
/// it is created.
pub type ExecutionEnginePort = Socket<TransactionList, ()>;

/// The port which upon receiving a delivery acknowledgment can add it to the aggregator
/// queue which will later roll up a batch of delivery acknowledgments to the consensus.
pub type DeliveryAcknowledgmentPort = Socket<DeliveryAcknowledgment, ()>;

#[async_trait]
pub trait ConsensusInterface: WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync {
    /// Create a new consensus service with the provided config and executor.
    async fn init(
        config: Self::Config,
        executor: Socket<TransactionList, ()>,
    ) -> anyhow::Result<Self>;

    /// Returns a port that can be used to submit transactions to the consensus,
    /// this can be used by any other systems that are interested in posting some
    /// transaction to the consensus.
    fn mempool(&self) -> MempoolPort;
}

#[async_trait]
pub trait ApplicationInterface:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    /// Create a new instance of the application layer using the provided configuration.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns a port that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEnginePort`].
    fn transaction_executor(&self) -> ExecutionEnginePort;
}

pub struct DeliveryAcknowledgment;

#[async_trait]
pub trait DeliveryAcknowledgmentAggregatorInterface:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    ///
    async fn init(config: Self::Config, consensus: MempoolPort) -> anyhow::Result<Self>;

    ///
    fn port(&self) -> DeliveryAcknowledgmentPort;
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Weight {
    /// It's good to know, but not crucial information.
    Relaxed,
    /// A strong behavior.
    Strong,
    /// Has the strongest weight, usually used for provably wrong behavior.
    Provable,
}

pub struct PeerId;

#[async_trait]
pub trait ReputationAggregatorInterface: Clone {
    fn new() -> anyhow::Result<Self>;

    fn report_sat(peer_id: &PeerId, weight: Weight);

    fn report_unsat(peer_id: &PeerId, weight: Weight);
}

pub struct Blake3Hash(pub [u8; 32]);
pub struct Blake3Tree(pub Vec<[u8; 32]>);

#[async_trait]
pub trait BlockStoreInterface: Clone + Send + Sync + ConfigConsumer {
    type SharedPointer<T: ?Sized>: Deref<Target = T> + Clone + Send + Sync;

    /// Create a new block store from the given configuration values.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>>;

    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
    ) -> Option<Self::SharedPointer<[u8]>>;

    async fn put_tree(&self, tree: &Blake3Tree);

    async fn put_block(&self, block_counter: u32, data: &[u8]);
}

/// The abstraction layer for different origins and how we handle them in the codebase in
/// a modular way, and [`OriginProvider`] can be something like a provider for resolving
/// *IPFS* files.
#[async_trait]
pub trait OriginProvider<Stream: tokio_stream::Stream<Item = bytes::BytesMut>> {
    ///
    fn fetch(&self, uri: &[u8]) -> Stream;
}

pub struct Node<
    ConfigProvider: ConfigProviderInterface,
    Consensus: ConsensusInterface,
    Application: ApplicationInterface,
    BlockStore: BlockStoreInterface,
    Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
    DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
> {
    pub configuration: ConfigProvider,
    pub consensus: Consensus,
    pub application: Application,
    pub block_store: BlockStore,
    pub origin_providers: HashMap<String, Box<dyn OriginProvider<Stream>>>,
    pub delivery_acknowledgment_aggregator: DeliveryAcknowledgmentAggregator,
}

impl<
        ConfigProvider: ConfigProviderInterface,
        Consensus: ConsensusInterface,
        Application: ApplicationInterface,
        BlockStore: BlockStoreInterface,
        Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
        DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
    >
    Node<
        ConfigProvider,
        Consensus,
        Application,
        BlockStore,
        Stream,
        DeliveryAcknowledgmentAggregator,
    >
{
    pub async fn init(configuration: ConfigProvider) -> anyhow::Result<Self> {
        let block_store = BlockStore::init(configuration.get::<BlockStore>()).await?;

        let application = Application::init(configuration.get::<Application>()).await?;

        let consensus = Consensus::init(
            configuration.get::<Consensus>(),
            application.transaction_executor(),
        )
        .await?;

        let delivery_acknowledgment_aggregator = DeliveryAcknowledgmentAggregator::init(
            configuration.get::<DeliveryAcknowledgmentAggregator>(),
            consensus.mempool(),
        )
        .await?;

        Ok(Self {
            configuration,
            consensus,
            application,
            block_store,
            origin_providers: HashMap::new(),
            delivery_acknowledgment_aggregator,
        })
    }

    pub fn register_origin_provider(
        &mut self,
        name: String,
        provider: Box<dyn OriginProvider<Stream>>,
    ) {
        if self.origin_providers.insert(name, provider).is_some() {
            panic!("Duplicate origin provider.");
        }
    }

    /// Returns true if the node is in a healt
    pub fn is_healthy(&self) -> bool {
        let application_status = self.application.is_running();
        let consensus_status = self.consensus.is_running();
        let aggregator_status = self.delivery_acknowledgment_aggregator.is_running();

        (application_status == consensus_status) && (consensus_status == aggregator_status)
    }
}

#[async_trait]
impl<
        ConfigProvider: ConfigProviderInterface,
        Consensus: ConsensusInterface,
        Application: ApplicationInterface,
        BlockStore: BlockStoreInterface,
        Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
        DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
    > WithStartAndShutdown
    for Node<
        ConfigProvider,
        Consensus,
        Application,
        BlockStore,
        Stream,
        DeliveryAcknowledgmentAggregator,
    >
where
    Self: Send,
    for<'a> &'a Self: Send,
{
    fn is_running(&self) -> bool {
        self.application.is_running()
            && self.consensus.is_running()
            && self.delivery_acknowledgment_aggregator.is_running()
    }

    async fn start(&self) {
        self.application.start().await;
        self.consensus.start().await;
        self.delivery_acknowledgment_aggregator.start().await;
    }

    async fn shutdown(&self) {
        self.application.shutdown().await;
        self.consensus.shutdown().await;
        self.delivery_acknowledgment_aggregator.shutdown().await;
    }
}
