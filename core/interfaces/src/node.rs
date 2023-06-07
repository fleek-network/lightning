use std::{collections::HashMap, marker::PhantomData};

use async_trait::async_trait;

use crate::{
    application::ApplicationInterface,
    blockstore::BlockStoreInterface,
    common::WithStartAndShutdown,
    config::ConfigProviderInterface,
    consensus::ConsensusInterface,
    fs::FileSystemInterface,
    handshake::HandshakeInterface,
    indexer::IndexerInterface,
    origin::OriginProviderInterface,
    pod::DeliveryAcknowledgmentAggregatorInterface,
    reputation::ReputationAggregatorInterface,
    rpc::RpcInterface,
    sdk::{HandlerFn, SdkInterface},
    signer::SignerInterface,
    types::ServiceId,
};

pub struct Node<
    ConfigProvider: ConfigProviderInterface,
    Consensus: ConsensusInterface,
    Application: ApplicationInterface,
    BlockStore: BlockStoreInterface,
    Indexer: IndexerInterface,
    FileSystem: FileSystemInterface<BlockStore = BlockStore, Indexer = Indexer>,
    Signer: SignerInterface,
    Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
    DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
    ReputationAggregator: ReputationAggregatorInterface,
    Rpc: RpcInterface,
    Sdk: SdkInterface<
        SyncQuery = Application::SyncExecutor,
        ReputationReporter = ReputationAggregator::ReputationReporter,
        FileSystem = FileSystem,
    >,
    Handshake: HandshakeInterface<Sdk = Sdk>,
> {
    pub configuration: ConfigProvider,
    pub consensus: Consensus,
    pub application: Application,
    pub store: BlockStore,
    pub indexer: Indexer,
    pub fs: FileSystem,
    pub signer: Signer,
    pub origin_providers: HashMap<String, Box<dyn OriginProviderInterface<Stream>>>,
    pub rpc: Rpc,
    pub delivery_acknowledgment_aggregator: DeliveryAcknowledgmentAggregator,
    pub reputation_aggregator: ReputationAggregator,
    pub handshake: Handshake,
    pub sdk: PhantomData<Sdk>,
}

impl<
    ConfigProvider: ConfigProviderInterface,
    Consensus: ConsensusInterface,
    Application: ApplicationInterface,
    BlockStore: BlockStoreInterface,
    Indexer: IndexerInterface,
    FileSystem: FileSystemInterface<BlockStore = BlockStore, Indexer = Indexer>,
    Signer: SignerInterface,
    Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
    DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
    ReputationAggregator: ReputationAggregatorInterface,
    Rpc: RpcInterface,
    Sdk: SdkInterface<
        SyncQuery = Application::SyncExecutor,
        ReputationReporter = ReputationAggregator::ReputationReporter,
        FileSystem = FileSystem,
    >,
    Handshake: HandshakeInterface<Sdk = Sdk>,
>
    Node<
        ConfigProvider,
        Consensus,
        Application,
        BlockStore,
        Indexer,
        FileSystem,
        Signer,
        Stream,
        DeliveryAcknowledgmentAggregator,
        ReputationAggregator,
        Rpc,
        Sdk,
        Handshake,
    >
{
    pub async fn init(configuration: ConfigProvider) -> anyhow::Result<Self> {
        let mut signer = Signer::init(configuration.get::<Signer>()).await?;

        let application = Application::init(configuration.get::<Application>()).await?;

        let consensus = Consensus::init(
            configuration.get::<Consensus>(),
            &signer,
            application.transaction_executor(),
            application.sync_query(),
        )
        .await?;

        // Provide the mempool socket to the signer so it can use it to send messages to consensus.
        signer.provide_mempool(consensus.mempool());

        let store = BlockStore::init(configuration.get::<BlockStore>()).await?;

        let indexer = Indexer::init(configuration.get::<Indexer>()).await?;

        let fs = FileSystem::new(&store, &indexer);

        let delivery_acknowledgment_aggregator = DeliveryAcknowledgmentAggregator::init(
            configuration.get::<DeliveryAcknowledgmentAggregator>(),
            signer.get_socket(),
        )
        .await?;

        let reputation_aggregator = ReputationAggregator::init(
            configuration.get::<ReputationAggregator>(),
            signer.get_socket(),
        )
        .await?;

        let rpc = Rpc::init(
            configuration.get::<Rpc>(),
            consensus.mempool(),
            application.query_socket(),
        )
        .await?;

        let handshake = Handshake::init(configuration.get::<Handshake>()).await?;

        Ok(Self {
            configuration,
            consensus,
            application,
            store,
            indexer,
            fs,
            signer,
            origin_providers: HashMap::new(),
            rpc,
            delivery_acknowledgment_aggregator,
            reputation_aggregator,
            handshake,
            sdk: PhantomData,
        })
    }

    pub fn register_origin_provider(
        &mut self,
        name: String,
        provider: Box<dyn OriginProviderInterface<Stream>>,
    ) {
        if self.origin_providers.insert(name, provider).is_some() {
            panic!("Duplicate origin provider.");
        }
    }

    pub fn register_service<S: FnOnce(Sdk) -> HandlerFn<'static, Sdk>>(
        &mut self,
        id: ServiceId,
        setup: S,
    ) {
        let sdk = Sdk::new(
            self.application.sync_query(),
            self.reputation_aggregator.get_reporter(),
            self.fs.clone(),
            self.signer.get_socket(),
        );

        let handler = setup(sdk.clone());

        self.handshake
            .register_service_request_handler(id, sdk, handler);
    }

    /// Returns true if the node is in a healthy.
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
    Indexer: IndexerInterface,
    FileSystem: FileSystemInterface<BlockStore = BlockStore, Indexer = Indexer>,
    Signer: SignerInterface,
    Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
    DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
    ReputationAggregator: ReputationAggregatorInterface,
    Rpc: RpcInterface,
    Sdk: SdkInterface<
        SyncQuery = Application::SyncExecutor,
        ReputationReporter = ReputationAggregator::ReputationReporter,
        FileSystem = FileSystem,
    >,
    Handshake: HandshakeInterface<Sdk = Sdk>,
> WithStartAndShutdown
    for Node<
        ConfigProvider,
        Consensus,
        Application,
        BlockStore,
        Indexer,
        FileSystem,
        Signer,
        Stream,
        DeliveryAcknowledgmentAggregator,
        ReputationAggregator,
        Rpc,
        Sdk,
        Handshake,
    >
where
    Self: Send,
    for<'a> &'a Self: Send,
{
    fn is_running(&self) -> bool {
        self.application.is_running()
            && self.consensus.is_running()
            && self.delivery_acknowledgment_aggregator.is_running()
            && self.indexer.is_running()
            && self.handshake.is_running()
    }

    async fn start(&self) {
        self.application.start().await;
        self.consensus.start().await;
        self.delivery_acknowledgment_aggregator.start().await;
        self.indexer.start().await;
        self.handshake.start().await;
    }

    async fn shutdown(&self) {
        self.application.shutdown().await;
        self.consensus.shutdown().await;
        self.delivery_acknowledgment_aggregator.shutdown().await;
        self.indexer.shutdown().await;
        self.handshake.shutdown().await;
    }
}
