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
use lightning_interfaces::types::Topic;
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
    SyncQueryRunnerInterface,
    TopologyInterface,
};
use tokio::sync::mpsc;

use crate::command::{Command, CommandReceiver};
use crate::db::Database;
use crate::frame::Frame;
use crate::interner::Interner;
use crate::peers::Peers;
use crate::recv_buffer::RecvBuffer;
use crate::ring::MessageRing;
use crate::{Advr, Message, Want};

// TODO(qti3e): Move this to somewhere else.
pub type Topology = Arc<Vec<Vec<NodePublicKey>>>;

// The connection pool sender and receiver types.
type S<C> = PoolSender<C, c![C::ConnectionPoolInterface], Frame>;
type R<C> = PoolReceiver<C, c![C::ConnectionPoolInterface], Frame>;

struct Context<C: Collection> {
    /// Our database where we store what we have seen.
    db: Database,
    /// Our digest interner.
    interner: Interner,
    /// Managers of incoming message queue for each topic.
    incoming_messages: [RecvBuffer; 3],
    /// The state related to the connected peers that we have right now.
    peers: Peers<S<C>, R<C>>,
    /// We use this socket to let the main event loop know that we have established
    /// a connection with another node.
    new_outgoing_connection_tx: mpsc::UnboundedSender<(S<C>, R<C>)>,
    /// The receiver end of the above socket.
    new_outgoing_connection_rx: mpsc::UnboundedReceiver<(S<C>, R<C>)>,
    /// The channel which sends the commands to the event loop.
    command_receiver: CommandReceiver,
    sqr: c![C::ApplicationInterface::SyncExecutor],
    notifier: c![C::NotifierInterface],
    topology: c![C::TopologyInterface],
    listener: c![C::ConnectionPoolInterface::Listener<Frame>],
    connector: c![C::ConnectionPoolInterface::Connector<Frame>],
}

impl<C: Collection> Context<C> {
    pub fn new(
        sqr: c![C::ApplicationInterface::SyncExecutor],
        notifier: c![C::NotifierInterface],
        topology: c![C::TopologyInterface],
        listener: c![C::ConnectionPoolInterface::Listener<Frame>],
        connector: c![C::ConnectionPoolInterface::Connector<Frame>],
    ) -> Self {
        todo!()
    }

    fn apply_topology(&mut self, _new_topology: Topology) {
        // TODO(qti3e)
    }

    fn handle_advr(&mut self, sender: NodePublicKey, advr: Advr) {
        // TODO(qti3e)
    }

    fn handle_want(&mut self, sender: NodePublicKey, req: Want) {
        // TODO(qti3e)
    }

    fn handle_message(&mut self, sender: NodePublicKey, msg: Message) {
        // TODO(qti3e)
    }

    /// Handle a message sent from a user.
    fn handle_frame(&mut self, sender: NodePublicKey, frame: Frame) {
        match frame {
            Frame::Advr(advr) => {
                self.handle_advr(sender, advr);
            },
            Frame::Want(want) => {
                self.handle_want(sender, want);
            },
            Frame::Message(msg) => {
                self.handle_message(sender, msg);
            },
        }
    }

    /// Handle a command sent from the mainland. Can be the broadcast object or
    /// a pubsub object.
    fn handle_command(&mut self, command: Command) {
        match command {
            Command::Recv(cmd) => {
                let index = topic_to_index(cmd.topic);
                self.incoming_messages[index].response_to(cmd.last_seen, cmd.response);
            },
            Command::Send(_) => todo!(),
            Command::Propagate(_) => todo!(),
            Command::MarkInvalidSender(_) => todo!(),
        }
    }
}

/// Runs the main loop of the broadcast algorithm. This is our main central worker.
async fn main_loop<C: Collection>(
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
    mut ctx: Context<C>,
) -> Context<C> {
    // Subscribe to the changes from the topology.
    let mut topology_subscriber =
        spawn_topology_subscriber::<C>(ctx.notifier.clone(), ctx.topology.clone());

    loop {
        tokio::select! {
            // We kind of care about the priority of these events. Or do we?
            // TODO(qti3e): Evaluate this.
            // biased;

            // Prioritize the shutdown signal over everything.
            _ = &mut shutdown => {
                break;
            },

            // A command has been sent from the mainland. Process it.
            Some(command) = ctx.command_receiver.recv() => {
                ctx.handle_command(command);
            },

            // The `topology_subscriber` recv can potentially return `None` when the
            // application execution socket is dropped. At that point we're also shutting
            // down anyway.
            Some(new_topology) = topology_subscriber.recv() => {
                ctx.apply_topology(new_topology);
            },

            Some(conn) = ctx.new_outgoing_connection_rx.recv() => {
                let Some(index) = ctx.sqr.pubkey_to_index(*conn.0.pk()) else {
                    continue;
                };
                ctx.peers.handle_new_outgoing_connection(index, conn);
            },

            Some((sender, frame)) = ctx.peers.recv() => {
                ctx.handle_frame(sender, frame);
            },

            // Handle the case when another node is dialing us.
            // TODO(qti3e): Is this cancel safe?
            Some(conn) = ctx.listener.accept() => {
                let Some(index) = ctx.sqr.pubkey_to_index(*conn.0.pk()) else {
                    continue;
                };
                ctx.peers.handle_new_incoming_connection(index, conn);
            },
        }
    }

    // Handover over the context.
    ctx
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
) -> mpsc::Receiver<Topology> {
    // Create the output channel from which we send out the computed
    // topology.
    let (w_tx, w_rx) = mpsc::channel(64);

    // Subscribe to new epochs coming in.
    let (tx, mut rx) = mpsc::channel(64);
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

/// Map each topic to a fixed size number.
#[inline(always)]
fn topic_to_index(topic: Topic) -> usize {
    match topic {
        Topic::Consensus => 0,
        Topic::DistributedHashTable => 1,
        Topic::Debug => 2,
    }
}
