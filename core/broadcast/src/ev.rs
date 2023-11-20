//! This file contains the implementation of the main event loop of the broadcast. You
//! can think of broadcast as a single-threaded event loop. This allows the broadcast
//! to hold mutable references to its internal state and avoid lock.
//!
//! Given most events being handled here are mutating a shared object. We can avoid any
//! implementation complexity or performance drops due to use of locks by just handling
//! the entire thing in a central event loop.

use std::cell::OnceCell;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use fleek_crypto::{NodePublicKey, NodeSecretKey, NodeSignature, PublicKey, SecretKey};
use infusion::c;
use ink_quill::ToDigest;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::{Digest, NodeIndex, Topic};
use lightning_interfaces::{
    ApplicationInterface,
    EventHandlerInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    ReputationReporterInterface,
    SyncQueryRunnerInterface,
    Weight,
};
use lightning_metrics::{histogram, increment_counter};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace};

use crate::command::{Command, CommandReceiver, CommandSender, SharedMessage};
use crate::db::Database;
use crate::frame::Frame;
use crate::interner::Interner;
use crate::pending::PendingStore;
use crate::recv_buffer::RecvBuffer;
use crate::ring::MessageRing;
use crate::stats::{ConnectionStats, Stats};
use crate::{Advr, Message, MessageInternedId, Want};

/// An interned id. But not from our interned table.
pub type RemoteInternedId = MessageInternedId;

/// The execution context of the broadcast.
pub struct Context<C: Collection> {
    /// Our database where we store what we have seen.
    db: Database,
    /// Our digest interner.
    interner: Interner,
    /// Managers of incoming message queue for each topic.
    incoming_messages: [RecvBuffer; 4],
    /// The state related to the connected peers that we have right now. Currently
    /// we only store the interned id mapping of us to their interned id.
    peers: im::HashMap<NodeIndex, im::HashMap<MessageInternedId, RemoteInternedId>>,
    /// The instance of stats collector.
    stats: Stats,
    /// The channel which sends the commands to the event loop.
    command_tx: CommandSender,
    /// Receiving end of the commands.
    command_rx: CommandReceiver,
    // Pending store.
    pending_store: PendingStore,
    sqr: c![C::ApplicationInterface::SyncExecutor],
    rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    event_handler: c![C::PoolInterface::EventHandler],
    sk: NodeSecretKey,
    pk: NodePublicKey,
    current_node_index: OnceCell<NodeIndex>,
}

impl<C: Collection> Context<C> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Database,
        sqr: c![C::ApplicationInterface::SyncExecutor],
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
        event_handler: c![C::PoolInterface::EventHandler],
        sk: NodeSecretKey,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let pk = sk.to_pk();
        Self {
            db,
            interner: Interner::new(u16::MAX),
            incoming_messages: [
                MessageRing::new(2048).into(),
                MessageRing::new(32).into(),
                MessageRing::new(1024).into(),
                MessageRing::new(1).into(),
            ],
            peers: im::HashMap::default(),
            stats: Stats::default(),
            command_tx,
            command_rx,
            pending_store: PendingStore::default(),
            sqr,
            rep_reporter,
            event_handler,
            sk,
            pk,
            current_node_index: OnceCell::new(), // will be set upon spawn.
        }
    }

    pub fn get_command_sender(&self) -> CommandSender {
        self.command_tx.clone()
    }

    /// Spawn the event loop returning a oneshot sender which can be used by the caller,
    /// when it is time for us to shutdown and exit the event loop.
    ///
    /// The context is preserved and not destroyed on the shutdown. And can be retrieved
    /// again by awaiting the `JoinHandle`.
    pub fn spawn(self) -> (oneshot::Sender<()>, JoinHandle<Self>) {
        let (shutdown_tx, shutdown) = oneshot::channel();
        let handle = tokio::spawn(async move {
            info!("spawning broadcast event loop");
            let tmp = main_loop(shutdown, self).await;
            info!("broadcast shut down sucessfully");
            tmp
        });
        (shutdown_tx, handle)
    }

    #[inline]
    fn get_node_pk(&self, index: NodeIndex) -> Option<NodePublicKey> {
        self.sqr.index_to_pubkey(index)
    }

    /// Handle a message sent from another node.
    fn handle_frame_payload(&mut self, sender: NodeIndex, payload: Bytes) {
        let Ok(frame) = Frame::decode(&payload) else {
            self.stats.report(sender, ConnectionStats {
                invalid_messages_received_from_peer: 1,
                ..Default::default()
            });
            return;
        };

        debug!("recivied frame '{frame:?}' from {sender}");

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

    fn handle_advr(&mut self, sender: NodeIndex, advr: Advr) {
        let digest = advr.digest;

        // If we have already propagated a message we really don't care about it anymore.
        if self.db.is_propagated(&digest) {
            trace!("skipping {digest:?}");
            return;
        }

        // Otherwise we might be interested in the message. We assign an interned id to this
        // digest if it already doesn't have one.
        let mut new_digest = false;
        let index = self.db.get_id(&digest).unwrap_or_else(|| {
            new_digest = true;
            let id = self.interner.insert(digest);
            self.db.insert_id(id, digest);
            id
        });

        // This is a new digest which we have never seen before. At this point we immediately
        // wanna see the message and therefore we send out the want request.
        if new_digest {
            self.send(
                sender,
                Frame::Want(Want {
                    interned_id: advr.interned_id,
                }),
            );
            self.pending_store.insert_request(sender, index);
        }

        // Remember the mapping since we may need it.
        self.peers
            .entry(sender)
            .or_default()
            .insert(index, advr.interned_id);

        // Handle the case for non-first advr.
        self.pending_store.insert_pending(sender, index);
    }

    fn handle_want(&mut self, sender: NodeIndex, req: Want) {
        trace!("got want from {sender} for {}", req.interned_id);
        let id = req.interned_id;
        let Some(digest) = self.interner.get(id) else {
            trace!("invalid interned id {id}");
            return;
        };
        let Some(message) = self.db.get_message(digest) else {
            error!("failed to find message");
            return;
        };
        self.send(sender, Frame::Message(message));
    }

    fn handle_message(&mut self, sender: NodeIndex, msg: Message) {
        let Some(origin_pk) = self.get_node_pk(msg.origin) else {
            self.stats.report(sender, ConnectionStats {
                invalid_messages_received_from_peer: 1,
                ..Default::default()
            });
            return;
        };

        let digest = msg.to_digest();

        let Some(id) = self.db.get_id(&digest) else {
            // We got a message without being advertised first. Although we can technically
            // accept the message. It is against the protocol. We should report.
            //
            // Accepting it will also further complicate the edge cases related to assigning
            // an interned id to the message, all of which is not supposed to be happening at
            // this step.
            self.stats.report(
                sender,
                ConnectionStats {
                    unwanted_messages_received_from_peer: 1,
                    ..Default::default()
                },
            );
            return;
        };

        if !origin_pk.verify(&msg.signature, &digest) {
            self.stats.report(
                sender,
                ConnectionStats {
                    invalid_messages_received_from_peer: 1,
                    ..Default::default()
                },
            );
            return;
        }

        let topic_index = topic_to_index(msg.topic);

        let shared = SharedMessage {
            digest,
            origin: msg.origin,
            payload: msg.payload.clone().into(),
        };

        let now = now();
        // only insert metrics for pseudo-valid timestamps (not in the future)
        if msg.timestamp > now {
            increment_counter!(
                "broadcast_message_received_count",
                Some("Counter for messages received over time")
            );
            histogram!(
                "broadcast_message_received_time",
                Some("Time taken to receive messages from the originator"),
                (msg.timestamp - now) as f64 / 1000.
            );
        }

        // Insert the message into the database for future lookups.
        self.db.insert_message(&digest, msg);
        // Mark message as received for RTT measurements.
        self.pending_store.received_message(sender, id);
        // Report a satisfactory interaction when we receive a message.
        self.rep_reporter.report_sat(sender, Weight::Weak);
        self.incoming_messages[topic_index].insert(shared);
    }

    /// Handle a command sent from the mainland. Can be the broadcast object or
    /// a pubsub object.
    fn handle_command(&mut self, command: Command) {
        debug!("handling broadcast command {command:?}");

        match command {
            Command::Recv(cmd) => {
                let index = topic_to_index(cmd.topic);
                self.incoming_messages[index].respond_to_recv_request(cmd.last_seen, cmd.response);
            },
            Command::Send(cmd) => {
                let node_index = self
                    .sqr
                    .pubkey_to_index(self.pk)
                    .expect("Tried to send message before node index was available");
                let node_index = self.current_node_index.get_or_init(|| node_index);

                let (digest, message) = {
                    let mut tmp = Message {
                        origin: *node_index,
                        signature: NodeSignature([0; 64]),
                        topic: cmd.topic,
                        timestamp: now(),
                        payload: cmd.payload,
                    };
                    let digest = tmp.to_digest();
                    tmp.signature = self.sk.sign(&digest);
                    (digest, tmp)
                };

                let id = self.interner.insert(digest);
                self.db.insert_with_message(id, digest, message);

                // Start advertising the message.
                self.advertise(id, digest, cmd.filter);
            },
            Command::Propagate(cmd) => {
                let Some(id) = self.db.get_id(&cmd.digest) else  {
                    debug_assert!(
                        false,
                        "We should not be trying to propagate a message we don't know the id of."
                    );
                    return;
                };

                // Mark the message as propagated. This will make us lose any interest for future
                // advertisements of this message.
                self.db.mark_propagated(&cmd.digest);

                // Remove the received message from the pending store.
                self.pending_store.remove_message(id);

                // Continue with advertising this message to the connected peers.
                self.advertise(id, cmd.digest, cmd.filter);

                increment_counter!(
                    "broadcast_messages_propagated",
                    Some("Number of messages we have initialized propagation to our peers for.")
                )
            },
            Command::MarkInvalidSender(_digest) => {
                error!("Received message from invalid sender");
                // TODO(qti3e): There is more to do here.
            },
        }
    }

    #[inline]
    fn advertise(
        &self,
        interned_id: MessageInternedId,
        digest: Digest,
        filter: Option<HashSet<NodeIndex>>,
    ) {
        let mut message = Vec::new();
        if Frame::Advr(Advr {
            interned_id,
            digest,
        })
        .encode(&mut message)
        .is_err()
        {
            error!("failed to encode advertisement");
            return;
        };

        // TODO(qti3e): If there are too many connections consider spawning node here.

        match filter {
            Some(nodes) => self
                .event_handler
                .send_to_all(message.into(), move |id| nodes.contains(&id)),
            None => {
                let peers = self.peers.clone();
                self.event_handler.send_to_all(message.into(), move |id| {
                    peers
                        .get(&id)
                        .map(|mapping| !mapping.contains_key(&interned_id))
                        .unwrap_or(true)
                });
            },
        }
    }

    #[inline]
    fn send(&self, destination: NodeIndex, frame: Frame) {
        let mut message = Vec::new();
        if frame.encode(&mut message).is_err() {
            error!("failed to encode outgoing message.");
            return;
        };

        self.event_handler.send_to_one(destination, message.into());
    }

    fn handle_pending_tick(&self, requests: Vec<(MessageInternedId, NodeIndex)>) {
        for (our_interned_id, node_index) in requests {
            if let Some(&interned_id) = self
                .peers
                .get(&node_index)
                .and_then(|c| c.get(&our_interned_id))
            {
                self.send(node_index, Frame::Want(Want { interned_id }))
            }
        }
    }
}

/// Runs the main loop of the broadcast algorithm. This is our main central worker.
async fn main_loop<C: Collection>(
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
    mut ctx: Context<C>,
) -> Context<C> {
    info!("Starting broadcast for {}", ctx.pk);

    // During initialization the application state might have not had loaded, and at
    // that point we might not have inserted the node index of the currently running
    // node. Let's give it another shot here.
    //
    // We need the node index because when we're sending messages out (when the current
    // node is the origin of the message.), we have to put our own id as the origin.
    if let Some(node_index) = ctx.sqr.pubkey_to_index(ctx.pk) {
        let _ = ctx.current_node_index.set(node_index);
    }

    loop {
        debug!("waiting for next event.");
        tokio::select! {
            // We kind of care about the priority of these events. Or do we?
            // TODO(qti3e): Evaluate this.
            biased;

            // Prioritize the shutdown signal over everything.
            _ = &mut shutdown => {
                info!("exiting event loop");
                break;
            },

            // A command has been sent from the mainland. Process it.
            Some(command) = ctx.command_rx.recv() => {
                trace!("received command in event loop");
                ctx.handle_command(command);
            },
            Some((sender, payload)) = ctx.event_handler.receive() => {
                ctx.handle_frame_payload(sender, payload);
            }
            requests = ctx.pending_store.tick() => {
                ctx.handle_pending_tick(requests);
            }
        }
    }

    // Handover over the context.
    ctx
}

/// Map each topic to a fixed size number.
#[inline(always)]
fn topic_to_index(topic: Topic) -> usize {
    match topic {
        Topic::Consensus => 0,
        Topic::DistributedHashTable => 1,
        Topic::Resolver => 2,
        Topic::Debug => 3,
    }
}

/// Get the current unix timestamp in milliseconds
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
