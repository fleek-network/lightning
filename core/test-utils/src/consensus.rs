use std::collections::HashSet;
use std::marker::PhantomData;
use std::time::Duration;

use affair::{AsyncWorker, Executor, TokioSpawn};
use fdi::{BuildGraph, Cloned, DependencyGraph, MethodExt};
use lightning_interfaces::types::{Block, TransactionRequest};
use lightning_interfaces::{
    c,
    ApplicationInterface,
    Collection,
    ConfigConsumer,
    ConfigProviderInterface,
    ConsensusInterface,
    Emitter,
    ExecutionEngineSocket,
    ForwarderInterface,
    MempoolSocket,
    NotifierInterface,
    ShutdownWaiter,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::time::{interval, sleep};

/// A mock forwarder that sends transactions. MUST ALWAYS be used alongside [`MockConsensus`].
pub struct MockForwarder<C>(MempoolSocket, PhantomData<C>);

impl<C: Collection> ForwarderInterface<C> for MockForwarder<C> {
    fn mempool_socket(&self) -> MempoolSocket {
        self.0.clone()
    }
}

impl<C: Collection> BuildGraph for MockForwarder<C> {
    fn build_graph() -> DependencyGraph {
        DependencyGraph::new().with_infallible(|consensus: &MockConsensus<C>| {
            MockForwarder::<C>(consensus.socket.clone(), PhantomData)
        })
    }
}

/// Mock consensus. MUST ALWAYS be used with [`MockForwarder`]
#[allow(clippy::type_complexity)]
pub struct MockConsensus<C: Collection> {
    socket: MempoolSocket,
    executor: ExecutionEngineSocket,
    notifier: c![C::NotifierInterface::Emitter],
    config: Config,
}

struct MockConsensusWorker<NE: Emitter> {
    executor: ExecutionEngineSocket,
    config: Config,
    notifier: NE,
    tx_count: u32,
}

impl<C: Collection> ConsensusInterface<C> for MockConsensus<C> {
    type Certificate = ();
}

impl<C: Collection> ConfigConsumer for MockConsensus<C> {
    const KEY: &'static str = "consensus";
    type Config = Config;
}

impl<C: Collection> BuildGraph for MockConsensus<C> {
    fn build_graph() -> DependencyGraph {
        DependencyGraph::new()
            .with_infallible(Self::new.on("start", fdi::consume(Self::start).spawn()))
    }
}

impl<C: Collection> MockConsensus<C> {
    /// Create a new consensus service with the provided config and executor.
    pub fn new(
        config: &C::ConfigProviderInterface,
        app: &C::ApplicationInterface,
        notifier: &c!(C::NotifierInterface),
    ) -> Self {
        let config = config.get::<Self>();
        let executor = app.transaction_executor();
        let notifier = notifier.get_emitter();

        let worker = MockConsensusWorker {
            executor: executor.clone(),
            config: config.clone(),
            notifier: notifier.clone(),
            tx_count: 0,
        };

        let socket = TokioSpawn::spawn_async(worker);

        Self {
            socket,
            executor,
            config,
            notifier,
        }
    }

    async fn start(self, Cloned(waiter): Cloned<ShutdownWaiter>) {
        let mut interval = interval(self.config.new_block_interval);
        waiter
            .run_until_shutdown(async move {
                loop {
                    interval.tick().await;

                    let block = Block {
                        transactions: vec![],
                        digest: [0; 32],
                    };

                    let response = self
                        .executor
                        .run(block.clone())
                        .await
                        .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
                        .unwrap();

                    self.notifier.new_block(block, response);
                }
            })
            .await;
    }
}

impl<NE: Emitter> AsyncWorker for MockConsensusWorker<NE> {
    type Request = TransactionRequest;
    type Response = ();

    async fn handle(&mut self, task: Self::Request) {
        // Randomly wait before ordering the transaction to make it more realistic.
        let range = self.config.min_ordering_time..self.config.max_ordering_time;
        let ordering_duration = rand::thread_rng().gen_range(range);
        sleep(Duration::from_secs(ordering_duration)).await;

        // Randomly drop a transaction so we can handle this case.
        if rand::thread_rng().gen_bool(self.config.probability_txn_lost) {
            return;
        }

        self.tx_count += 1;

        if self.config.transactions_to_lose.contains(&self.tx_count) {
            return;
        }

        let block = Block {
            transactions: vec![task],
            digest: [0; 32],
        };

        let response = self
            .executor
            .run(block.clone())
            .await
            .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
            .unwrap();

        self.notifier.new_block(block, response);
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// Lower bound for the random time it takes to order a transaction.
    pub min_ordering_time: u64,
    /// Upper bound for the random time it takes to order a transaction.
    pub max_ordering_time: u64,
    /// Probability that a transaction won't get through.
    /// The nonce won't be incremented on the application.
    pub probability_txn_lost: f64,
    /// Transactions specified in this set will be lost.
    /// For example, if the set contains 1 and 3, then the first and third transactions
    /// arriving at the consensus will be lost.
    pub transactions_to_lose: HashSet<u32>,
    /// This specifies the interval for new blocks being pretend submitted to the application.
    pub new_block_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_ordering_time: 0,
            max_ordering_time: 5,
            probability_txn_lost: 0.1,
            transactions_to_lose: HashSet::new(),
            new_block_interval: Duration::from_secs(5),
        }
    }
}
