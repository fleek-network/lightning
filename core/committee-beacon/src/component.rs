use std::marker::PhantomData;

use anyhow::Result;
use lightning_interfaces::prelude::*;

use crate::config::CommitteeBeaconConfig;
use crate::database::CommitteeBeaconDatabase;
use crate::listener::CommitteeBeaconListener;
use crate::rocks::RocksCommitteeBeaconDatabase;
use crate::timer::CommitteeBeaconTimer;
use crate::CommitteeBeaconQuery;

pub struct CommitteeBeaconComponent<C: NodeComponents> {
    db: RocksCommitteeBeaconDatabase,
    _components: PhantomData<C>,
}

/// The committee beacon is a top-level node component, so it must implement `BuildGraph` to
/// integrate with the node's dependency injection framework.
impl<C: NodeComponents> BuildGraph for CommitteeBeaconComponent<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(Self::init.with_event_handler("start", Self::start))
    }
}

/// The committee beacon has a configuration within the node's configuration, which is declared by
/// this implementation.
impl<C: NodeComponents> ConfigConsumer for CommitteeBeaconComponent<C> {
    const KEY: &'static str = "committee-beacon";

    type Config = CommitteeBeaconConfig;
}

impl<C: NodeComponents> CommitteeBeaconComponent<C> {
    /// Initialize the committee beacon.
    ///
    /// This method is called once during node initialization and includes the dependant node
    /// components as arguments, along with the node configuration, which includes our committee
    /// beacon configuration.
    fn init(config: &C::ConfigProviderInterface) -> Result<Self> {
        let config = config.get::<Self>();
        let db = RocksCommitteeBeaconDatabase::build(config.database);
        Ok(Self {
            db,
            _components: PhantomData,
        })
    }

    /// Spawn and start the committee beacon listener, returning immediately without blocking.
    fn start(
        &self,
        forwarder: &C::ForwarderInterface,
        fdi::Cloned(keystore): fdi::Cloned<C::KeystoreInterface>,
        fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
        fdi::Cloned(app_query): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
        shutdown: fdi::Cloned<ShutdownWaiter>,
    ) -> Result<()> {
        tracing::debug!("spawning committee beacon");

        let db = self.db.clone();
        let listener_mempool_socket = forwarder.mempool_socket();
        let timer_mempool_socket = forwarder.mempool_socket();
        let listener_shutdown = shutdown.clone();
        let timer_shutdown = shutdown.clone();
        spawn!(
            async move {
                app_query.wait_for_genesis().await;

                // Start the listener.
                let listener = CommitteeBeaconListener::<C>::new(
                    db,
                    keystore.clone(),
                    notifier.clone(),
                    app_query.clone(),
                    listener_mempool_socket,
                )
                .expect("failed to create committee beacon listener");
                let listener_waiter = listener_shutdown.clone();
                spawn!(
                    async move {
                        if let Some(result) =
                            listener_shutdown.run_until_shutdown(listener.start()).await
                        {
                            result.expect("committee beacon listener failed")
                        }
                    },
                    "COMMITTEE-BEACON listener",
                    crucial(listener_waiter)
                );

                // Start the timer.
                let timer = CommitteeBeaconTimer::<C>::new(
                    keystore,
                    notifier,
                    app_query,
                    timer_mempool_socket,
                )
                .expect("failed to create committee beacon timer");
                let timer_waiter = timer_shutdown.clone();
                spawn!(
                    async move {
                        if let Some(result) = timer_shutdown.run_until_shutdown(timer.start()).await
                        {
                            result.expect("committee beacon timer failed")
                        }
                    },
                    "COMMITTEE-BEACON timer",
                    crucial(timer_waiter)
                );
            },
            "COMMITTEE-BEACON",
            crucial(shutdown)
        );

        Ok(())
    }
}

impl<C: NodeComponents> CommitteeBeaconInterface<C> for CommitteeBeaconComponent<C> {
    type Query = CommitteeBeaconQuery;

    fn query(&self) -> Self::Query {
        CommitteeBeaconQuery::new(self.db.query())
    }
}
