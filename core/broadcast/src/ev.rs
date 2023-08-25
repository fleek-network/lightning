//! This file contains the implementation of the main event loop of the broadcast. You
//! can think of broadcast as a single-threaded event loop. This allows the broadcast
//! to hold mutable references to its internal state and avoid lock.
//!
//! Given most events being handled here are mutating a shared object. We can avoid any
//! implementation complexity or performance drops due to use of locks by just handling
//! the entire thing in a central event loop.

use std::sync::Arc;

use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ApplicationInterface,
    ConnectionPoolInterface,
    ListenerConnector,
    ListenerInterface,
    NotifierInterface,
    PoolReceiver,
    PoolSender,
    SenderInterface,
    SenderReceiver,
    TopologyInterface,
};

use crate::db::Database;
use crate::frame::Frame;
use crate::interner::Interner;
use crate::peers::Peers;

// TODO(qti3e): Move this to somewhere else.
pub type Topology = Arc<Vec<Vec<NodePublicKey>>>;

// The connection pool sender and receiver types.
type S<C> = PoolSender<C, c![C::ConnectionPoolInterface], Frame>;
type R<C> = PoolReceiver<C, c![C::ConnectionPoolInterface], Frame>;

struct State<C: Collection> {
    db: Database,
    interner: Interner,
    peers: Peers<S<C>, R<C>>,
}

impl<C: Collection> State<C> {
    fn apply_topology(&mut self, _new_topology: Topology) {}

    fn handle_new_incoming_connection(
        &mut self,
        (_sender, _receiver): SenderReceiver<C, c![C::ConnectionPoolInterface], Frame>,
    ) {
    }
}

pub async fn main_loop<C: Collection>(
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
    db: Database,
    _sqr: c![C::ApplicationInterface::SyncExecutor],
    notifier: c![C::NotifierInterface],
    topology: c![C::TopologyInterface],
    (mut listener, _connector): ListenerConnector<C, c![C::ConnectionPoolInterface], Frame>,
) -> c![C::ConnectionPoolInterface::Listener<Frame>] {
    let mut state = State::<C> {
        db,
        interner: Interner::new(1024),
        peers: Peers::default(),
    };

    // Subscribe to the changes from the topology.
    let mut topology_subscriber = spawn_topology_subscriber::<C>(notifier, topology);

    loop {
        tokio::select! {
            // We kind of care about the priority of these events. Or do we?
            // TODO(qti3e): Evaluate this.
            // biased;

            // Prioritize the shutdown signal over everything.
            _ = &mut shutdown => {
                break;
            },

            // The `topology_subscriber` recv can potentially return `None` when the
            // application execution socket is dropped. At that point we're also shutting
            // down anyway.
            Some(new_topology) = topology_subscriber.recv() => {
                state.apply_topology(new_topology);
            },

            // Handle the case when another node is dialing us.
            Some(conn) = listener.accept() => {
                state.handle_new_incoming_connection(conn);
            },
        }
    }

    // Handover the listener back once we're shutting down.
    listener
}

/// Spawn a task that listens for epoch changes and sends the new topology through an `mpsc`
/// channel.
///
/// This function will always compute/fire the current topology as the first message before waiting
/// for the next epoch.
///
/// This function does not block the caller and immediately returns.
fn spawn_topology_subscriber<C: Collection>(
    notifier: c![C::NotifierInterface],
    topology: c![C::TopologyInterface],
) -> tokio::sync::mpsc::Receiver<Topology> {
    // Create the output channel from which we send out the computed
    // topology.
    let (w_tx, w_rx) = tokio::sync::mpsc::channel(64);

    // Subscribe to new epochs coming in.
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    notifier.notify_on_new_epoch(tx);

    tokio::spawn(async move {
        'spell: loop {
            let topology = topology.clone();

            // Computing the topology might be a blocking task.
            let result = tokio::task::spawn_blocking(move || topology.suggest_connections());

            let topology = result.await.expect("Failed to compute topology.");

            if w_tx.send(topology).await.is_err() {
                // Nobody is interested in hearing what we have to say anymore.
                // This can happen when all instance of receivers are dropped.
                break 'spell;
            }

            // Wait until the next epoch.
            if rx.recv().await.is_none() {
                // Notifier has dropped the sender. Based on the current implementation of
                // different things, this can happen when the application's execution socket
                // is dropped.
                return;
            }
        }
    });

    w_rx
}
