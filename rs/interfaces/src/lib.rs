use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    ops::Deref,
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

/// An implementer of this trait should handle providing the configurations from
/// the loaded configuration file.
pub trait ConfigProviderInterface {
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

#[async_trait]
pub trait Port<Message>: Clone + Send + Sync {
    type Error: Debug + Display;

    async fn send(&self, message: Message) -> Result<(), Self::Error>;
}

pub struct TransactionList {}

pub trait WithStartAndShutdown {
    /// Returns true if this run
    fn is_running(&self) -> bool;

    fn start(&self);

    fn shutdown(&self);
}

#[async_trait]
pub trait ConsensusInterface: WithStartAndShutdown + ConfigConsumer + Sized {
    type Port<M>: Port<M>;

    /// Create a new consensus service with the provided config and executor.
    async fn init(
        config: Self::Config,
        executor: impl Port<TransactionList>,
    ) -> anyhow::Result<Self>;

    /// Returns a port that can be used to submit transactions to the consensus.
    fn port(&self) -> Self::Port<TransactionList>;
}

#[async_trait]
pub trait ApplicationInterface: WithStartAndShutdown + ConfigConsumer + Sized {
    type Port<M>: Port<M>;

    /// Create a new instance of the application layer using the provided configuration.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns a port that should be used to submit transactions to be executed
    /// by the application layer.
    fn port(&self) -> Self::Port<TransactionList>;
}

pub struct DeliveryAcknowledgment;

#[async_trait]
pub trait DeliveryAcknowledgmentAggregatorInterface:
    WithStartAndShutdown + ConfigConsumer + Sized
{
    type Port<M>: Port<M>;

    async fn init(
        config: Self::Config,
        consensus: impl Port<TransactionList>,
    ) -> anyhow::Result<Self>;

    fn port(&self) -> Self::Port<DeliveryAcknowledgment>;
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
    type Port<M>: Port<M>;

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

#[async_trait]
pub trait OriginProvider<Stream: tokio_stream::Stream<Item = bytes::BytesMut>> {
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

        let consensus =
            Consensus::init(configuration.get::<Consensus>(), application.port()).await?;

        let delivery_acknowledgment_aggregator = DeliveryAcknowledgmentAggregator::init(
            configuration.get::<DeliveryAcknowledgmentAggregator>(),
            consensus.port(),
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
}

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
{
    fn is_running(&self) -> bool {
        self.application.is_running() && self.consensus.is_running()
    }

    fn start(&self) {
        self.application.start();
        self.consensus.start();
    }

    fn shutdown(&self) {
        self.application.shutdown();
        self.consensus.shutdown();
    }
}
