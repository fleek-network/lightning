//! This file contains the implementation of the main event loop of the broadcast. You
//! can think of broadcast as a single-threaded event loop. This allows the broadcast
//! to hold mutable references to its internal state and avoid lock.
//!
//! Given most events being handled here are mutating a shared object. We can avoid any
//! implementation complexity or performance drops due to use of locks by just handling
//! the entire thing in a central event loop.

use std::cell::OnceCell;
use std::collections::{HashSet, VecDeque};

use bytes::Bytes;
use fleek_crypto::NodeSignature;
use ink_quill::ToDigest;
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::broadcast::{Advr, Frame, Message, MessageInternedId, Want};
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::{Digest, NodeIndex, Topic};
use lightning_interfaces::Weight;
use lightning_metrics::{histogram, increment_counter};
use tokio::pin;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace};

use crate::backend::LightningBackend;
use crate::command::{Command, CommandReceiver, CommandSender, SharedMessage};
use crate::db::Database;
use crate::interner::Interner;
use crate::pending::PendingStore;
use crate::recv_buffer::RecvBuffer;
use crate::ring::MessageRing;
use crate::stats::{ConnectionStats, Stats};
use crate::BroadcastBackend;

/// An interned id. But not from our interned table.
pub type RemoteInternedId = MessageInternedId;

/// The execution context of the broadcast.
pub struct Context<B: BroadcastBackend> {
    /// Our database where we store what we have seen.
    db: Database,
    /// Our digest interner.
    interner: Interner,
    /// Managers of incoming message queue for each topic.
    ///
    /// NOTE: If a topic is added, this size needs to be updated.
    incoming_messages: [RecvBuffer; 5],
    /// The state related to the connected peers that we have right now. Currently
    /// we only store the interned id mapping of us to their interned id.
    peers: im::HashMap<NodeIndex, im::HashMap<MessageInternedId, RemoteInternedId>>,
    /// The instance of stats collector.
    stats: Stats,
    /// The channel which sends the commands to the event loop.
    command_tx: CommandSender,
    /// Receiving end of the commands.
    command_rx: CommandReceiver,
    /// Pending store.
    pending_store: PendingStore<B>,
    /// Incoming messages with the same digest
    processing: im::HashMap<Digest, VecDeque<MessageWithSender>>,
    current_node_index: OnceCell<NodeIndex>,
    backend: B,
}

/// Map each topic to a fixed size number.
///
/// NOTE: If a topic is added, the `incoming_messages` array size needs to be updated.
/// Compilation will NOT fail if this is not updated, but the node will panic at runtime with an
/// index out of bounds error.
#[inline(always)]
fn topic_to_index(topic: Topic) -> usize {
    match topic {
        Topic::Consensus => 0,
        Topic::Resolver => 1,
        Topic::Debug => 2,
        Topic::TaskBroker => 3,
        Topic::Checkpoint => 4,
    }
}

impl<B: BroadcastBackend> Context<B> {
    pub fn new(db: Database, backend: B) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        Self {
            db,
            interner: Interner::new(Interner::MAX_CAPACITY),
            incoming_messages: [
                MessageRing::new(2048).into(),
                MessageRing::new(32).into(),
                MessageRing::new(1024).into(),
                MessageRing::new(1).into(),
                MessageRing::new(123).into(),
            ],
            peers: im::HashMap::default(),
            stats: Stats::default(),
            command_tx,
            command_rx,
            pending_store: PendingStore::new(),
            processing: im::HashMap::new(),
            current_node_index: OnceCell::new(), // will be set upon spawn.
            backend,
        }
    }

    pub fn get_command_sender(&self) -> CommandSender {
        self.command_tx.clone()
    }

    /// Handle a message sent from another node.
    fn handle_frame_payload(&mut self, sender: NodeIndex, payload: Bytes) {
        let Ok(frame) = Frame::decode(&payload) else {
            self.stats.report(
                sender,
                ConnectionStats {
                    invalid_messages_received_from_peer: 1,
                    ..Default::default()
                },
            );
            return;
        };

        debug!("received frame '{frame:?}' from {sender}");

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
        if self.db.contains_message(&digest) {
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
        let digest = msg.to_digest();

        if self.db.contains_message(&digest) {
            // we have seen the message and propagated it already.
            return;
        }

        let Some(origin_pk) = self.backend.get_node_pk(msg.origin) else {
            self.stats.report(
                sender,
                ConnectionStats {
                    invalid_messages_received_from_peer: 1,
                    ..Default::default()
                },
            );
            return;
        };

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

        if !B::verify(&origin_pk, &msg.signature, &digest) {
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

        let now = Self::now();
        // only insert metrics for pseudo-valid timestamps (not in the future)
        if msg.timestamp < now {
            increment_counter!(
                "broadcast_message_received_count",
                Some("Counter for messages received over time")
            );
            histogram!(
                "broadcast_message_received_time",
                Some("Time taken to receive messages from the originator"),
                (now - msg.timestamp) as f64 / 1000.
            );
        }

        let msg_with_sender = MessageWithSender {
            message: msg,
            sender,
        };
        match self.processing.entry(digest) {
            im::hashmap::Entry::Vacant(e) => {
                let mut q = VecDeque::new();
                q.push_back(msg_with_sender);
                e.insert(q);
            },
            im::hashmap::Entry::Occupied(mut e) => {
                e.get_mut().push_back(msg_with_sender);
                return;
            },
        }

        // only make the message available to receive for pubsub if we aren't currently
        // processing a message with the same digest
        self.incoming_messages[topic_index].insert(shared);

        // Mark message as received for RTT measurements.
        self.pending_store.received_message(sender, id);
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
                    .backend
                    .get_our_index()
                    .expect("Tried to send message before node index was available");
                let node_index = self.current_node_index.get_or_init(|| node_index);

                let (digest, message) = {
                    let mut tmp = Message {
                        origin: *node_index,
                        signature: NodeSignature([0; 64]),
                        topic: cmd.topic,
                        timestamp: Self::now(),
                        payload: cmd.payload,
                    };
                    let digest = tmp.to_digest();
                    tmp.signature = self.backend.sign(digest);
                    (digest, tmp)
                };

                let _ = cmd.response.send(digest);

                if self.db.contains_message(&digest) {
                    // If we already received and accepted the message, we also advertised it.
                    // We don't need to send again.
                    return;
                }

                let id = if let Some(id) = self.db.get_id(&digest) {
                    // If the id exist already in the db, we have to check if it is still valid.
                    // An id is valid if the interner still has a digest for it, and this digest
                    // matches the message digest.
                    match self.interner.get(id) {
                        Some(existing_digest) if *existing_digest == digest => {
                            // no need to update the id
                            id
                        },
                        _ => {
                            let id = self.interner.insert(digest);
                            self.db.update_id(id, &digest);
                            id
                        },
                    }
                } else {
                    // If the db does not contain the id, we can simply assign a new one and insert
                    // it into the db.
                    let id = self.interner.insert(digest);
                    self.db.insert_id(id, digest);
                    id
                };
                self.db.insert_message(&digest, message);

                // Start advertising the message.
                self.advertise(id, digest, cmd.filter);
            },
            Command::CleanUp(digest) => {
                let Some(id) = self.db.get_id(&digest) else {
                    debug_assert!(
                        false,
                        "We should not be trying to clean up a digest we don't know the id of."
                    );
                    return;
                };
                if self.processing.remove(&digest).is_none() {
                    debug_assert!(
                        false,
                        "We should not be trying to clean up a digest we don't have."
                    );
                }

                // Remove the received message from the pending store.
                self.pending_store.remove_message(id);
            },
            Command::Propagate(cmd) => {
                let Some(id) = self.db.get_id(&cmd.digest) else {
                    debug_assert!(
                        false,
                        "We should not be trying to propagate a message we don't know the id of."
                    );
                    return;
                };

                // If the message originated from this node, it won't be stored in `processing`.
                // Also, if we re-propagate a message, it won't be stored in `processing` anymore,
                // but will already be in the `db`.
                if !self.db.contains_message(&cmd.digest) {
                    let Some(mut q) = self.processing.remove(&cmd.digest) else {
                        debug_assert!(
                            false,
                            "We should not be trying to propagate a message we don't have."
                        );
                        return;
                    };
                    let Some(msg) = q.pop_front() else {
                        debug_assert!(
                            false,
                            "We should not be trying to propagate a message we don't have."
                        );
                        return;
                    };
                    // Report a satisfactory interaction when we receive a message.
                    self.backend.report_sat(msg.sender, Weight::Weak);

                    // Insert the message into the database for future lookups.
                    self.db.insert_message(&cmd.digest, msg.message);
                }

                // Remove the received message from the pending store.
                self.pending_store.remove_message(id);

                // Continue with advertising this message to the connected peers.
                self.advertise(id, cmd.digest, cmd.filter);

                increment_counter!(
                    "broadcast_messages_propagated",
                    Some("Number of messages we have initialized propagation to our peers for.")
                )
            },
            Command::MarkInvalidSender(digest) => {
                error!("Received message from invalid sender");
                let Some(q) = self.processing.get_mut(&digest) else {
                    debug_assert!(
                        false,
                        "We should not be trying to reject a message we don't have."
                    );
                    return;
                };
                // Remove invalid message
                q.pop_front();

                // Move on to the next message in the queue if one exists.
                if let Some(next_msg) = q.front() {
                    let topic_index = topic_to_index(next_msg.message.topic);
                    let shared = SharedMessage {
                        digest,
                        origin: next_msg.message.origin,
                        payload: next_msg.message.payload.clone().into(),
                    };
                    self.incoming_messages[topic_index].insert(shared);
                }
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
                .backend
                .send_to_all(message.into(), move |id| nodes.contains(&id)),
            None => {
                let peers = self.peers.clone();
                self.backend.send_to_all(message.into(), move |id| {
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

        self.backend.send_to_one(destination, message.into());
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

    fn now() -> u64 {
        B::now()
    }

    pub async fn run(mut self, waiter: ShutdownWaiter) -> Self {
        info!("Starting broadcast for node");

        // During initialization the application state might have not had loaded, and at
        // that point we might not have inserted the node index of the currently running
        // node. Let's give it another shot here.
        //
        // We need the node index because when we're sending messages out (when the current
        // node is the origin of the message.), we have to put our own id as the origin.
        if let Some(node_index) = self.backend.get_our_index() {
            let _ = self.current_node_index.set(node_index);
        }

        let shutdown = waiter.into_future();
        pin!(shutdown);

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
                Some(command) = self.command_rx.recv() => {
                    trace!("received command in event loop");
                    self.handle_command(command);
                },
                Some((sender, payload)) = self.backend.receive() => {
                    self.handle_frame_payload(sender, payload);
                }
                requests = self.pending_store.tick() => {
                    self.handle_pending_tick(requests);
                }
            }
        }

        // Handover over the context.
        self
    }
}

impl<C: Collection> Context<LightningBackend<C>> {
    /// Spawn the event loop returning a oneshot sender which can be used by the caller,
    /// when it is time for us to shutdown and exit the event loop.
    ///
    /// The context is preserved and not destroyed on the shutdown. And can be retrieved
    /// again by awaiting the `JoinHandle`.
    pub fn spawn(self, waiter: ShutdownWaiter)
    where
        Self: Send,
    {
        let panic_waiter = waiter.clone();
        spawn!(
            async move {
                self.run(waiter).await;
            },
            "BROADCAST",
            crucial(panic_waiter)
        );
    }
}

#[derive(Clone)]
struct MessageWithSender {
    message: Message,
    sender: NodeIndex,
}
