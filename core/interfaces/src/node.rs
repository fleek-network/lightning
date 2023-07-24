use std::{collections::HashMap, marker::PhantomData, sync::Arc};

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
    notifier::NotifierInterface,
    origin::OriginProviderInterface,
    pod::DeliveryAcknowledgmentAggregatorInterface,
    reputation::ReputationAggregatorInterface,
    rpc::RpcInterface,
    sdk::{HandlerFn, SdkInterface},
    signer::SignerInterface,
    types::ServiceId,
    GossipInterface, TopologyInterface,
};

pub trait DracoTypes: Send + Sync {
    type ConfigProvider: ConfigProviderInterface;
    type Consensus: ConsensusInterface<
        QueryRunner = <Self::Application as ApplicationInterface>::SyncExecutor,
        PubSub = <Self::Gossip as GossipInterface>::PubSub<
            <Self::Consensus as ConsensusInterface>::Certificate,
        >,
    >;
    type Application: ApplicationInterface;
    type BlockStore: BlockStoreInterface;
    type Indexer: IndexerInterface;
    type FileSystem: FileSystemInterface<BlockStore = Self::BlockStore, Indexer = Self::Indexer>;
    type Signer: SignerInterface<
        SyncQuery = <Self::Application as ApplicationInterface>::SyncExecutor,
    >;
    type Stream: tokio_stream::Stream<Item = bytes::BytesMut>;
    type DeliveryAcknowledgmentAggregator: DeliveryAcknowledgmentAggregatorInterface;
    type Notifier: NotifierInterface<
        SyncQuery = <Self::Application as ApplicationInterface>::SyncExecutor,
    >;
    type ReputationAggregator: ReputationAggregatorInterface<Notifier = Self::Notifier>;
    type Rpc: RpcInterface<<Self::Application as ApplicationInterface>::SyncExecutor>;
    type Sdk: SdkInterface<
        SyncQuery = <Self::Application as ApplicationInterface>::SyncExecutor,
        ReputationReporter = <
            Self::ReputationAggregator as ReputationAggregatorInterface
        >::ReputationReporter,
        FileSystem = Self::FileSystem,
    >;
    type Handshake: HandshakeInterface<Sdk = Self::Sdk>;
    type Topology: TopologyInterface<
        SyncQuery = <Self::Application as ApplicationInterface>::SyncExecutor,
    >;
    type Gossip: GossipInterface<
        Topology = Self::Topology,
        Notifier = Self::Notifier,
        Signer = Self::Signer,
    >;
}

pub struct Node<T: DracoTypes> {
    pub configuration: Arc<T::ConfigProvider>,
    pub consensus: T::Consensus,
    pub application: T::Application,
    pub store: T::BlockStore,
    pub indexer: T::Indexer,
    pub fs: T::FileSystem,
    pub signer: T::Signer,
    pub origin_providers: HashMap<String, Box<dyn OriginProviderInterface<T::Stream>>>,
    pub rpc: T::Rpc,
    pub delivery_acknowledgment_aggregator: T::DeliveryAcknowledgmentAggregator,
    pub reputation_aggregator: T::ReputationAggregator,
    pub handshake: T::Handshake,
    pub topology: Arc<T::Topology>,
    pub gossip: T::Gossip,
    pub sdk: PhantomData<T::Sdk>,
    pub notifier: PhantomData<T::Notifier>,
}

impl<T: DracoTypes> Node<T> {
    pub async fn init(configuration: Arc<T::ConfigProvider>) -> anyhow::Result<Self> {
        let application = T::Application::init(configuration.get::<T::Application>()).await?;

        let mut signer =
            T::Signer::init(configuration.get::<T::Signer>(), application.sync_query()).await?;

        let topology = Arc::new(
            T::Topology::init(
                configuration.get::<T::Topology>(),
                signer.get_bls_pk(),
                application.sync_query(),
            )
            .await?,
        );

        let gossip =
            T::Gossip::init(configuration.get::<T::Gossip>(), topology.clone(), &signer).await?;
        let consensus_pubsub = gossip.get_pubsub(crate::Topic::Consensus);

        let consensus = T::Consensus::init(
            configuration.get::<T::Consensus>(),
            &signer,
            application.transaction_executor(),
            application.sync_query(),
            consensus_pubsub,
        )
        .await?;

        // Provide the mempool socket to the signer so it can use it to send messages to consensus.
        signer.provide_mempool(consensus.mempool());
        signer.provide_new_block_notify(consensus.new_block_notifier());

        let store = T::BlockStore::init(configuration.get::<T::BlockStore>()).await?;

        let indexer = T::Indexer::init(configuration.get::<T::Indexer>()).await?;

        let fs = T::FileSystem::new(&store, &indexer);

        let delivery_acknowledgment_aggregator = T::DeliveryAcknowledgmentAggregator::init(
            configuration.get::<T::DeliveryAcknowledgmentAggregator>(),
            signer.get_socket(),
        )
        .await?;

        let notifier = T::Notifier::init(application.sync_query());

        let reputation_aggregator = T::ReputationAggregator::init(
            configuration.get::<T::ReputationAggregator>(),
            signer.get_socket(),
            notifier,
        )
        .await?;

        let rpc = T::Rpc::init(
            configuration.get::<T::Rpc>(),
            consensus.mempool(),
            application.sync_query(),
        )
        .await?;

        let handshake = T::Handshake::init(configuration.get::<T::Handshake>()).await?;

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
            topology,
            gossip,
            sdk: PhantomData,
            notifier: PhantomData,
        })
    }

    pub fn register_origin_provider(
        &mut self,
        name: String,
        provider: Box<dyn OriginProviderInterface<T::Stream>>,
    ) {
        if self.origin_providers.insert(name, provider).is_some() {
            panic!("Duplicate origin provider.");
        }
    }

    pub fn register_service<S: FnOnce(T::Sdk) -> HandlerFn<'static, T::Sdk>>(
        &mut self,
        id: ServiceId,
        setup: S,
    ) {
        let sdk = T::Sdk::new(
            self.application.sync_query(),
            self.reputation_aggregator.get_reporter(),
            self.fs.clone(),
            self.signer.get_socket(),
        );

        let handler = setup(sdk.clone());

        self.handshake
            .register_service_request_handler(id, sdk, handler);
    }

    /// Returns true if the node is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        let application_status = self.application.is_running();
        let consensus_status = self.consensus.is_running();
        let aggregator_status = self.delivery_acknowledgment_aggregator.is_running();

        (application_status == consensus_status) && (consensus_status == aggregator_status)
    }

    /// An associated function that consumes every
    pub fn fill_configuration(configuration: &T::ConfigProvider) {
        configuration.get::<T::Consensus>();
        configuration.get::<T::Application>();
        configuration.get::<T::BlockStore>();
        configuration.get::<T::Indexer>();
        configuration.get::<T::Signer>();
        configuration.get::<T::DeliveryAcknowledgmentAggregator>();
        configuration.get::<T::ReputationAggregator>();
        configuration.get::<T::Rpc>();
        configuration.get::<T::Handshake>();
    }
}

#[async_trait]
impl<T: DracoTypes> WithStartAndShutdown for Node<T>
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
        self.signer.start().await;
        self.consensus.start().await;
        self.delivery_acknowledgment_aggregator.start().await;
        self.indexer.start().await;
        self.handshake.start().await;
        self.rpc.start().await;
    }

    async fn shutdown(&self) {
        self.application.shutdown().await;
        self.consensus.shutdown().await;
        self.delivery_acknowledgment_aggregator.shutdown().await;
        self.indexer.shutdown().await;
        self.handshake.shutdown().await;
        self.rpc.shutdown().await;
    }
}

pub mod transformers {
    use super::*;

    #[rustfmt::skip]
    pub struct WithConsensus<
        T: DracoTypes,
        Consensus: ConsensusInterface<
            QueryRunner = <T::Application as ApplicationInterface>::SyncExecutor
                >,
            >(T, PhantomData<Consensus>);

    impl<
        T: DracoTypes,
        Consensus: ConsensusInterface<
            QueryRunner = <T::Application as ApplicationInterface>::SyncExecutor,
            PubSub = <T::Gossip as GossipInterface>::PubSub<
                <Consensus as ConsensusInterface>::Certificate,
            >,
        >,
    > DracoTypes for WithConsensus<T, Consensus>
    {
        type ConfigProvider = T::ConfigProvider;
        type Consensus = Consensus;
        type Application = T::Application;
        type BlockStore = T::BlockStore;
        type Indexer = T::Indexer;
        type FileSystem = T::FileSystem;
        type Signer = T::Signer;
        type Stream = T::Stream;
        type DeliveryAcknowledgmentAggregator = T::DeliveryAcknowledgmentAggregator;
        type Notifier = T::Notifier;
        type ReputationAggregator = T::ReputationAggregator;
        type Rpc = T::Rpc;
        type Sdk = T::Sdk;
        type Handshake = T::Handshake;
        type Topology = T::Topology;
        type Gossip = T::Gossip;
    }
}
