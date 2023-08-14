use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::{
    application::ApplicationInterface,
    blockstore::BlockStoreInterface,
    common::WithStartAndShutdown,
    config::ConfigProviderInterface,
    consensus::ConsensusInterface,
    dht::DhtInterface,
    handshake::HandshakeInterface,
    notifier::NotifierInterface,
    origin::OriginProviderSocket,
    pod::DeliveryAcknowledgmentAggregatorInterface,
    reputation::ReputationAggregatorInterface,
    rpc::RpcInterface,
    signer::SignerInterface,
    types::{ServiceScope, Topic},
    BroadcastInterface, ConnectionPoolInterface, TopologyInterface,
};

pub trait LightningTypes: Send + Sync {
    type ConfigProvider: ConfigProviderInterface;
    type Consensus: ConsensusInterface<
        QueryRunner = <Self::Application as ApplicationInterface>::SyncExecutor,
        PubSub = <Self::Broadcast as BroadcastInterface>::PubSub<
            <Self::Consensus as ConsensusInterface>::Certificate,
        >,
    >;
    type Application: ApplicationInterface;
    type BlockStore: BlockStoreInterface;
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
    type Handshake: HandshakeInterface;
    type Topology: TopologyInterface<
        SyncQuery = <Self::Application as ApplicationInterface>::SyncExecutor,
    >;
    type Broadcast: BroadcastInterface<
        Topology = Self::Topology,
        Notifier = Self::Notifier,
        Signer = Self::Signer,
        ConnectionPool = Self::ConnectionPool,
    >;
    type ConnectionPool: ConnectionPoolInterface<
        QueryRunner = <Self::Application as ApplicationInterface>::SyncExecutor,
        Signer = Self::Signer,
    >;
    type Dht: DhtInterface<Topology = Self::Topology>;
}

pub struct Node<T: LightningTypes> {
    pub configuration: Arc<T::ConfigProvider>,
    pub connection_pool: T::ConnectionPool,
    pub consensus: T::Consensus,
    pub application: T::Application,
    pub store: T::BlockStore,
    pub signer: T::Signer,
    pub origin_providers: HashMap<String, OriginProviderSocket<T::Stream>>,
    pub rpc: T::Rpc,
    pub delivery_acknowledgment_aggregator: T::DeliveryAcknowledgmentAggregator,
    pub reputation_aggregator: T::ReputationAggregator,
    pub handshake: T::Handshake,
    pub topology: Arc<T::Topology>,
    pub broadcast: T::Broadcast,
    pub dht: T::Dht,
    pub notifier: PhantomData<T::Notifier>,
}

impl<T: LightningTypes> Node<T> {
    pub fn init(configuration: Arc<T::ConfigProvider>) -> anyhow::Result<Self> {
        let application = T::Application::init(configuration.get::<T::Application>())?;

        let mut signer =
            T::Signer::init(configuration.get::<T::Signer>(), application.sync_query())?;

        let topology = Arc::new(T::Topology::init(
            configuration.get::<T::Topology>(),
            signer.get_bls_pk(),
            application.sync_query(),
        )?);

        let dht = T::Dht::init(&signer, topology.clone(), configuration.get::<T::Dht>())?;

        let notifier = T::Notifier::init(application.sync_query());

        let connection_pool = T::ConnectionPool::init(
            configuration.get::<T::ConnectionPool>(),
            &signer,
            application.sync_query(),
        );
        let broadcast_pool = connection_pool.bind(ServiceScope::Broadcast);

        let broadcast = T::Broadcast::init(
            configuration.get::<T::Broadcast>(),
            broadcast_pool,
            topology.clone(),
            &signer,
            notifier,
        )?;
        let consensus_pubsub = broadcast.get_pubsub(Topic::Consensus);

        let consensus = T::Consensus::init(
            configuration.get::<T::Consensus>(),
            &signer,
            application.transaction_executor(),
            application.sync_query(),
            consensus_pubsub,
        )?;

        // Provide the mempool socket to the signer so it can use it to send messages to consensus.
        signer.provide_mempool(consensus.mempool());
        signer.provide_new_block_notify(consensus.new_block_notifier());

        let store = T::BlockStore::init(configuration.get::<T::BlockStore>())?;

        let delivery_acknowledgment_aggregator = T::DeliveryAcknowledgmentAggregator::init(
            configuration.get::<T::DeliveryAcknowledgmentAggregator>(),
            signer.get_socket(),
        )?;

        let notifier = T::Notifier::init(application.sync_query());

        let reputation_aggregator = T::ReputationAggregator::init(
            configuration.get::<T::ReputationAggregator>(),
            signer.get_socket(),
            notifier,
        )?;

        let rpc = T::Rpc::init(
            configuration.get::<T::Rpc>(),
            consensus.mempool(),
            application.sync_query(),
        )?;

        #[cfg(feature = "e2e-test")]
        rpc.provide_dht_socket(dht.get_socket());

        let handshake = T::Handshake::init(configuration.get::<T::Handshake>())?;

        Ok(Self {
            configuration,
            connection_pool,
            consensus,
            application,
            store,
            signer,
            origin_providers: HashMap::new(),
            rpc,
            delivery_acknowledgment_aggregator,
            reputation_aggregator,
            handshake,
            topology,
            broadcast,
            dht,
            notifier: PhantomData,
        })
    }

    pub fn register_origin_provider(
        &mut self,
        name: String,
        provider: OriginProviderSocket<T::Stream>,
    ) {
        if self.origin_providers.insert(name, provider).is_some() {
            panic!("Duplicate origin provider.");
        }
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
        configuration.get::<T::Signer>();
        configuration.get::<T::DeliveryAcknowledgmentAggregator>();
        configuration.get::<T::ReputationAggregator>();
        configuration.get::<T::Rpc>();
        configuration.get::<T::Handshake>();
        configuration.get::<T::Dht>();
    }
}

#[async_trait]
impl<T: LightningTypes> WithStartAndShutdown for Node<T>
where
    Self: Send,
    for<'a> &'a Self: Send,
{
    fn is_running(&self) -> bool {
        self.application.is_running()
            && self.connection_pool.is_running()
            && self.consensus.is_running()
            && self.delivery_acknowledgment_aggregator.is_running()
            && self.handshake.is_running()
            && self.dht.is_running()
    }

    async fn start(&self) {
        self.application.start().await;
        self.connection_pool.start().await;
        self.signer.start().await;
        self.consensus.start().await;
        self.delivery_acknowledgment_aggregator.start().await;
        self.handshake.start().await;
        self.rpc.start().await;
        self.dht.start().await;
    }

    async fn shutdown(&self) {
        self.application.shutdown().await;
        self.connection_pool.shutdown().await;
        self.consensus.shutdown().await;
        self.delivery_acknowledgment_aggregator.shutdown().await;
        self.handshake.shutdown().await;
        self.rpc.shutdown().await;
        self.dht.shutdown().await;
    }
}

pub mod transformers {
    use super::*;

    #[rustfmt::skip]
    pub struct WithConsensus<
        T: LightningTypes,
        Consensus: ConsensusInterface<
            QueryRunner = <T::Application as ApplicationInterface>::SyncExecutor
                >,
            >(T, PhantomData<Consensus>);

    impl<
        T: LightningTypes,
        Consensus: ConsensusInterface<
            QueryRunner = <T::Application as ApplicationInterface>::SyncExecutor,
            PubSub = <T::Broadcast as BroadcastInterface>::PubSub<
                <Consensus as ConsensusInterface>::Certificate,
            >,
        >,
    > LightningTypes for WithConsensus<T, Consensus>
    {
        type ConfigProvider = T::ConfigProvider;
        type Consensus = Consensus;
        type Application = T::Application;
        type BlockStore = T::BlockStore;
        type Signer = T::Signer;
        type Stream = T::Stream;
        type DeliveryAcknowledgmentAggregator = T::DeliveryAcknowledgmentAggregator;
        type Notifier = T::Notifier;
        type ReputationAggregator = T::ReputationAggregator;
        type Rpc = T::Rpc;
        type Handshake = T::Handshake;
        type Topology = T::Topology;
        type Broadcast = T::Broadcast;
        type ConnectionPool = T::ConnectionPool;
        type Dht = T::Dht;
    }
}
