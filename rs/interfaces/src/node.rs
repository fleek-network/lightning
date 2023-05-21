use std::{collections::HashMap, marker::PhantomData};

use async_trait::async_trait;

use crate::{
    application::ApplicationInterface, blockstore::BlockStoreInterface,
    common::WithStartAndShutdown, config::ConfigProviderInterface, consensus::ConsensusInterface,
    identity::SignatureVerifierInterface, origin::OriginProviderInterface,
    pod::DeliveryAcknowledgmentAggregatorInterface, rpc::RpcInterface,
};

pub struct Node<
    ConfigProvider: ConfigProviderInterface,
    Consensus: ConsensusInterface,
    Application: ApplicationInterface,
    BlockStore: BlockStoreInterface,
    SignatureVerifier: SignatureVerifierInterface,
    Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
    DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
    Rpc: RpcInterface<SignatureVerifier>,
> {
    pub configuration: ConfigProvider,
    pub consensus: Consensus,
    pub application: Application,
    pub block_store: BlockStore,
    pub origin_providers: HashMap<String, Box<dyn OriginProviderInterface<Stream>>>,
    pub rpc: Rpc,
    pub delivery_acknowledgment_aggregator: DeliveryAcknowledgmentAggregator,
    pub signature_verifier: PhantomData<SignatureVerifier>,
}

impl<
        ConfigProvider: ConfigProviderInterface,
        Consensus: ConsensusInterface,
        Application: ApplicationInterface,
        BlockStore: BlockStoreInterface,
        SignatureVerifier: SignatureVerifierInterface,
        Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
        DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
        Rpc: RpcInterface<SignatureVerifier>,
    >
    Node<
        ConfigProvider,
        Consensus,
        Application,
        BlockStore,
        SignatureVerifier,
        Stream,
        DeliveryAcknowledgmentAggregator,
        Rpc,
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

        let rpc = Rpc::init(
            configuration.get::<Rpc>(),
            consensus.mempool(),
            application.query_port(),
        )
        .await?;

        Ok(Self {
            configuration,
            consensus,
            application,
            block_store,
            origin_providers: HashMap::new(),
            rpc,
            delivery_acknowledgment_aggregator,
            signature_verifier: PhantomData,
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
        SignatureVerifier: SignatureVerifierInterface,
        Stream: tokio_stream::Stream<Item = bytes::BytesMut>,
        DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface,
        Rpc: RpcInterface<SignatureVerifier>,
    > WithStartAndShutdown
    for Node<
        ConfigProvider,
        Consensus,
        Application,
        BlockStore,
        SignatureVerifier,
        Stream,
        DeliveryAcknowledgmentAggregator,
        Rpc,
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
