use std::cell::RefCell;
use std::pin::Pin;
use std::sync::Arc;

use dashmap::DashMap;
use derive_more::AddAssign;
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use futures::Future;
use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ReceiverInterface, SenderInterface, SyncQueryRunnerInterface};

use crate::conn::{create_pair, PairedReceiver, PairedSender};
use crate::ev::Topology;
use crate::frame::{Digest, Frame};
use crate::receivers::Receivers;
use crate::stats::{ConnectionStats, Stats};
use crate::{Advr, MessageInternedId};

/// This struct is responsible for holding the state of the current peers
/// that we are connected to.
pub struct Peers<S, R>
where
    S: SenderInterface<Frame>,
    R: ReceiverInterface<Frame>,
{
    /// The id of our node.
    us: NodeIndex,
    /// Our access to reporting stats.
    stats: Stats,
    /// Map each public key to the info we have about that peer.
    peers: im::HashMap<NodePublicKey, Peer<S>>,
    /// The message queue from all the connections we have.
    incoming_messages: Receivers<R>,
}

/// An interned id. But not from our interned table.
pub type RemoteInternedId = MessageInternedId;

struct Peer<S: SenderInterface<Frame>> {
    /// The index of the node.
    index: NodeIndex,
    /// The sender that we can use to send messages to this peer.
    sender: PairedSender<S>,
    /// The origin of this connection can tell us if we started this connection or if
    /// the remote has dialed us.
    origin: ConnectionOrigin,
    /// The status of our connection with this peer.
    status: ConnectionStatus,
    // We know this peer has these messages. They have advertised them to us before.
    //
    // Here we are storing the mapping from the interned id of a message in our table,
    // to the interned id of the message known to the client.
    has: im::HashMap<MessageInternedId, RemoteInternedId>,
}

impl<S: SenderInterface<Frame>> Clone for Peer<S> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            sender: self.sender.clone(),
            origin: self.origin,
            status: self.status,
            has: self.has.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum ConnectionOrigin {
    // We have established the connection.
    Us,
    /// The remote has dialed us and we have this connection because we got
    /// a connection from the listener.
    Remote,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionStatus {
    /// The connection with the other peer is open.
    ///
    /// We are sending advertisements to this node and actively listening for their
    /// advertisements.
    Open,
    /// The connection with this node is closing. We do not wish to interact with it
    /// anymore. But since we may have pending communication with them. We are still
    /// keeping the connection alive.
    ///
    /// At this point we do not care about their advertisements. We only care about
    /// the messages they owe us. Once the other peer does not owe us anything anymore
    /// we close the connection.
    Closing,
    /// The connection with the other peer is closed and we are not communicating with
    /// the node.
    Closed,
}

impl<S: SenderInterface<Frame>, R: ReceiverInterface<Frame>> Peers<S, R> {
    pub fn set_current_node_index(&mut self, index: NodeIndex) {
        self.us = index;
    }

    pub fn advertise(&self, id: MessageInternedId, digest: Digest) {
        let msg = Advr {
            interned_id: id,
            digest,
        };

        self.for_each(move |stats, (_pk, info)| {
            if info.has.contains_key(&id) {
                return;
            }

            stats.report(
                info.index,
                ConnectionStats {
                    advertisements_received_from_us: 1,
                    ..Default::default()
                },
            );

            tokio::spawn(async move {
                // TODO(qti3e): Explore allowing to send raw buffers from here.
                // There is a lot of duplicated serialization going on because
                // of this pattern.
                info.sender.send(Frame::Advr(msg)).await;
            });
        });
    }

    #[inline]
    fn for_each<F>(&self, closure: F)
    where
        F: Fn(&Stats, (NodePublicKey, Peer<S>)) + Send + 'static,
    {
        // Take a snapshot of the state. This is O(1).
        let state = self.peers.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            for (pk, info) in state {
                closure(&stats, (pk, info));
            }
        });
    }

    pub async fn handle_new_connection(
        &mut self,
        origin: ConnectionOrigin,
        index: NodeIndex,
        (sender, receiver): (S, R),
    ) {
        let (sender, receiver) = create_pair(sender, receiver);
        let pk = *sender.pk();

        if let Some(info) = self.peers.get(&pk) {}

        let info = Peer {
            index,
            sender,
            origin: ConnectionOrigin::Remote,
            status: ConnectionStatus::Open,
            has: Default::default(),
        };

        self.peers.insert(pk, info);
        self.incoming_messages.push(receiver);
    }

    pub async fn recv(&mut self) -> Option<(NodePublicKey, Frame)> {
        match self.incoming_messages.recv().await {
            Some((r, None)) => {
                self.disconnected(r);
                None
            },
            Some((receiver, Some(frame))) => self.handle_frame(receiver, frame),
            None => None,
        }
    }

    #[inline(always)]
    fn disconnected(&mut self, receiver: PairedReceiver<R>) {
        let pk = *receiver.pk();
        if let Some(mut info) = self.peers.remove(&pk) {
            if !receiver.is_receiver_of(&info.sender) {
                // We are using a different connection at this point,
                // insert it back and return. There is nothing for us
                // to do here.
                self.peers.insert(pk, info);
                return;
            }
            info.status = ConnectionStatus::Closed;
        }
    }

    #[inline(always)]
    fn handle_frame(
        &mut self,
        receiver: PairedReceiver<R>,
        frame: Frame,
    ) -> Option<(NodePublicKey, Frame)> {
        let pk = *receiver.pk();
        let info = self.peers.get(&pk)?;

        // Update the stats on what we got.
        self.stats.report(
            info.index,
            match frame {
                Frame::Advr(_) => ConnectionStats {
                    advertisements_received_from_peer: 1,
                    ..Default::default()
                },
                Frame::Want(_) => ConnectionStats {
                    wants_received_from_peer: 1,
                    ..Default::default()
                },
                Frame::Message(_) => ConnectionStats {
                    messages_received_from_peer: 1,
                    ..Default::default()
                },
            },
        );

        match (info.status, frame) {
            (ConnectionStatus::Open, frame) => {
                self.incoming_messages.push(receiver);
                Some((pk, frame))
            },
            (ConnectionStatus::Closed, _) => None,
            (ConnectionStatus::Closing, Frame::Message(msg)) => {
                if self.keep_alive(&pk) {
                    self.incoming_messages.push(receiver);
                }
                Some((pk, Frame::Message(msg)))
            },
            // If we're closing the connection with this peer, we don't care about
            // anything other than the actual payloads that they are still sending
            // our way.
            (ConnectionStatus::Closing, _) => {
                if self.keep_alive(&pk) {
                    self.incoming_messages.push(receiver);
                }
                None
            },
        }
    }

    /// Based on the number of messages the other node owes us. And the time elapsed since
    /// we decided to close the connection, returns a boolean indicating whether or not we
    /// are interested in keeping this connection going and if we should still keep listening
    /// for a message to come.
    #[inline(always)]
    fn keep_alive(&self, pk: &NodePublicKey) -> bool {
        // TODO(qti3e): Implement this function.
        true
    }
}

impl<S: SenderInterface<Frame>, R: ReceiverInterface<Frame>> Default for Peers<S, R> {
    fn default() -> Self {
        Self {
            us: 0,
            stats: Default::default(),
            peers: Default::default(),
            incoming_messages: Default::default(),
        }
    }
}
