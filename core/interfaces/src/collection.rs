use std::marker::PhantomData;
use std::time::Duration;

use anyhow::Result;
use fdi::Provider;
use tokio::time::sleep;

use super::*;

// Define the collection of every top-level trait in the system. The collections is basically
// a trait with a bunch of associated types called members. Each of them taking the entire
// collection as input.
collection!([
    ConfigProviderInterface,
    KeystoreInterface,
    ApplicationInterface,
    BlockstoreInterface,
    BlockstoreServerInterface,
    CheckpointerInterface,
    SyncronizerInterface,
    BroadcastInterface,
    TopologyInterface,
    ArchiveInterface,
    ForwarderInterface,
    ConsensusInterface,
    HandshakeInterface,
    NotifierInterface,
    OriginProviderInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    ReputationAggregatorInterface,
    ResolverInterface,
    RpcInterface,
    ServiceExecutorInterface,
    TaskBrokerInterface,
    SignerInterface,
    FetcherInterface,
    PoolInterface,
    PingerInterface,
    IndexerInterface,
]);

/// The Fleek Network node.
pub struct Node<C: Collection> {
    pub provider: Provider,
    pub shutdown: Option<ShutdownController>,
    _p: PhantomData<C>,
}

impl<C: Collection> Node<C> {
    #[inline(always)]
    pub fn init(config: C::ConfigProviderInterface) -> Result<Self> {
        let provider = Provider::default();
        provider.insert(config);
        Self::init_with_provider(provider)
    }

    pub fn init_with_provider(mut provider: Provider) -> Result<Self> {
        let mut exec = provider.get_mut::<fdi::Executor>();
        exec.set_spawn_cb(|fut, _name| {
            tokio::spawn(fut);
        });

        let graph = C::build_graph();

        // let vis = graph.viz("Lightning Dependency Graph");
        // println!("{vis}");

        graph.init_all(&mut provider)?;

        let shutdown = provider.take();

        Ok(Self {
            provider,
            shutdown: Some(shutdown),
            _p: PhantomData,
        })
    }

    pub async fn start(&self) {
        self.provider.trigger("start");
    }

    pub fn shutdown_waiter(&self) -> Option<ShutdownWaiter> {
        self.shutdown.as_ref().map(|s| s.waiter())
    }

    /// Shutdown the node
    pub async fn shutdown(&mut self) {
        let mut shutdown = self
            .shutdown
            .take()
            .expect("cannot call shutdown more than once");

        tracing::trace!("Shutting node down.");
        shutdown.trigger_shutdown();

        for i in 0.. {
            tokio::select! {
                biased;
                _ = shutdown.wait_for_completion() => {
                    return;
                },
                _ = sleep(Duration::from_secs(5)) => {
                    match i {
                        0 => {
                            tracing::trace!("Still shutting down...");
                            continue;
                        },
                        1 => {
                            tracing::warn!("Still shutting down...");
                            continue;
                        },
                        _ => {
                            tracing::error!("Shutdown taking too long")
                        }
                    }
                }
            }

            let Some(iter) = shutdown.pending_backtraces() else {
                continue;
            };

            eprintln!("Printing pending backtraces:");
            for (i, trace) in iter.enumerate() {
                eprintln!("Pending task backtrace #{i}:\n{trace:#?}");
            }
        }
    }
}
