use std::marker::PhantomData;

use anyhow::{Context, Result};
use lightning_interfaces::prelude::*;
use lightning_interfaces::CheckpointerInterface;
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;
use types::Topic;

use crate::attestation_listener::AttestationListener;
use crate::config::CheckpointerConfig;
use crate::database::CheckpointerDatabase;
use crate::epoch_change_listener::EpochChangeListener;
use crate::message::CheckpointBroadcastMessage;
use crate::rocks::RocksCheckpointerDatabase;
use crate::CheckpointerQuery;

/// The checkpointer is a component that produces checkpoint attestations and aggregates them when a
/// supermajority is reached.
///
/// It listens for epoch change notifications from the notifier and checkpoint attestation messages
/// from the broadcaster. It's responsible for creating checkpoint attestations and checking for a
/// supermajority of attestations for the epoch, aggregating the signatures if a supermajority is
/// reached, and saving the aggregate checkpoint to the database as the canonical checkpoint for the
/// epoch. The aggregate checkpoint contains a state root that can be used by clients to verify the
/// blockchain state using merkle proofs.
pub struct Checkpointer<C: Collection> {
    db: RocksCheckpointerDatabase,
    keystore: C::KeystoreInterface,
    pubsub: <C::BroadcastInterface as BroadcastInterface<C>>::PubSub<CheckpointBroadcastMessage>,
    notifier: C::NotifierInterface,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    ready: TokioReadyWaiter<()>,
    _collection: PhantomData<C>,
}

impl<C: Collection> Checkpointer<C> {
    pub fn new(
        db: RocksCheckpointerDatabase,
        keystore: &C::KeystoreInterface,
        broadcast: &C::BroadcastInterface,
        notifier: &C::NotifierInterface,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self {
            db,
            keystore: keystore.clone(),
            pubsub: broadcast.get_pubsub(Topic::Checkpoint),
            notifier: notifier.clone(),
            app_query,
            ready: Default::default(),
            _collection: PhantomData,
        }
    }

    /// Initialize the checkpointer.
    ///
    /// This method is called once during node initialization and includes the dependant node
    /// components as arguments, along with the node configuration, which includes our checkpointer
    /// configuration.
    fn init(
        config: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        broadcast: &C::BroadcastInterface,
        notifier: &C::NotifierInterface,
        fdi::Cloned(app_query): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Result<Self> {
        let config = config.get::<Self>();

        let db = RocksCheckpointerDatabase::build(config.database.clone());

        Ok(Self::new(db, keystore, broadcast, notifier, app_query))
    }

    /// Spawn the checkpointer.
    ///
    /// This method spawns the checkpointer and returns immediately. It does not block until the
    /// checkpointer is finished.
    fn spawn(this: fdi::Ref<Self>, shutdown: fdi::Cloned<ShutdownWaiter>) -> Result<()> {
        tracing::debug!("spawning checkpointer");

        let waiter = shutdown.clone();
        spawn!(
            async move { this.start(waiter).await.expect("checkpointer failed") },
            "CHECKPOINTER",
            crucial(shutdown)
        );

        Ok(())
    }

    /// Start the checkpointer.
    ///
    /// This method spawns the epoch change listener and the attestation listener, and blocks
    /// until they are both finished or if either one fails.
    async fn start(&self, shutdown: ShutdownWaiter) -> Result<()> {
        tracing::debug!("starting checkpointer");

        // Wait for genesis to be applied before starting the checkpointer.
        self.app_query.wait_for_genesis().await;

        // Get this node's index from the application query runner.
        let node_public_key = self.keystore.get_ed25519_pk();
        let node_id = self
            .app_query
            .pubkey_to_index(&node_public_key)
            .context("node's own public key not found in application state.")?;

        // Consume checkpoint attestations from the broadcaster.
        // Spawns a new tokio task and continues execution immediately.
        let _attestation_listener_handle = AttestationListener::<C>::new(
            self.db.clone(),
            self.pubsub.clone(),
            self.app_query.clone(),
        )
        .spawn(shutdown.clone());

        // Listen for epoch changes from the notifier.
        // Spawns a new tokio task and continues execution immediately.
        let (_, epoch_change_listener_ready) = EpochChangeListener::<C>::spawn(
            node_id,
            self.db.clone(),
            self.keystore.clone(),
            self.pubsub.clone(),
            self.notifier.clone(),
            self.app_query.clone(),
            shutdown.clone(),
        );

        // Wait for the epoch change listener to be ready and subscribe to the epoch changed
        // notifier.
        epoch_change_listener_ready.wait().await;

        // Notify that we are ready.
        self.ready.notify(());

        // Wait for shutdown.
        tracing::debug!("waiting for shutdown");
        shutdown.wait_for_shutdown().await;

        tracing::debug!("shutdown checkpointer");
        Ok(())
    }
}

/// The checkpointer has a configuration within the node's configuration, which is declared by this
/// implementation.
impl<C: Collection> ConfigConsumer for Checkpointer<C> {
    const KEY: &'static str = "checkpointer";

    type Config = CheckpointerConfig;
}

/// The checkpointer is a top-level node component, so it must implement `BuildGraph` to integrate
/// with the node's dependency injection framework.
impl<C: Collection> fdi::BuildGraph for Checkpointer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(Self::init.with_event_handler("start", Self::spawn))
    }
}

impl<C: Collection> CheckpointerInterface<C> for Checkpointer<C> {
    type ReadyState = ();
    type Query = CheckpointerQuery;

    /// Get a query instance for the checkpointer.
    fn query(&self) -> Self::Query {
        CheckpointerQuery::new(self.db.query())
    }

    /// Wait for the checkpointer to be ready.
    async fn wait_for_ready(&self) -> Self::ReadyState {
        self.ready.wait().await
    }
}
