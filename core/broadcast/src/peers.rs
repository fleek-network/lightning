use std::cell::RefCell;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

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
use crate::{Advr, Message, MessageInternedId, Want};

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
    pub stats: Stats,
    /// Peers that are pinned. These are connections suggested by the topology. We
    /// do not drop these connections when performing garbage collection.
    pinned: FxHashSet<NodePublicKey>,
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

/// The originator of a connection.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionOrigin {
    // We have established the connection.
    Us,
    /// The remote has dialed us and we have this connection because we got
    /// a connection from the listener.
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn get_node_index(&self, pk: &NodePublicKey) -> Option<NodeIndex> {
        self.peers.get(pk).map(|i| i.index)
    }

    pub fn set_current_node_index(&mut self, index: NodeIndex) {
        self.us = index;
    }

    /// Returns the status of the connection with the given peer. If no connection exists returns
    /// [`ConnectionStatus::Closed`].
    pub fn get_connection_status(&self, pk: &NodePublicKey) -> ConnectionStatus {
        self.peers
            .get(pk)
            .map(|e| e.status)
            .unwrap_or(ConnectionStatus::Closed)
    }

    /// If a connection is available returns the originator of the connection, otherwise `None`.
    pub fn get_connection_origin(&self, pk: &NodePublicKey) -> Option<ConnectionOrigin> {
        self.peers.get(pk).map(|e| e.origin)
    }

    /// Unpin every pinned connection.
    pub fn unpin_all(&mut self) {
        self.pinned.clear();
    }

    /// Pin a peer to prevent garbage collector to remove the connection.
    pub fn pin_peer(&mut self, pk: NodePublicKey) {
        self.pinned.insert(pk);
    }

    /// Move every connection made by us that is not pinned into closing state.
    pub fn disconnect_unpinned(&mut self) {
        // TODO(qti3e)
    }

    /// Insert a mapping from a local interned message id we have to the remote interned id for a
    /// given remote node.
    pub fn insert_index_mapping(
        &mut self,
        remote: &NodePublicKey,
        local_index: MessageInternedId,
        remote_index: RemoteInternedId,
    ) {
        let Some(info) = self.peers.get_mut(remote) else {
            return;
        };

        info.has.insert(local_index, remote_index);
    }

    /// Get the interned id a remote knows a message we know by our local interned id, or `None`.
    pub fn get_index_mapping(
        &self,
        remote: &NodePublicKey,
        local_index: MessageInternedId,
    ) -> Option<RemoteInternedId> {
        self.peers
            .get(remote)
            .and_then(|i| i.has.get(&local_index))
            .copied()
    }

    /// Send a `Frame::Message` to the specific node.
    // TODO(qti3e): Fix double serialization.
    pub fn send_message(&self, remote: &NodePublicKey, frame: Frame) {
        log::trace!("sending want response to {remote}");
        debug_assert!(matches!(frame, Frame::Message(_)));
        let Some(info) = self.peers.get(remote) else {
            return;
        };
        let sender = info.sender.clone();
        tokio::spawn(async move {
            sender.send(frame).await;
        });
    }

    /// Send a want request to the given node returns `None` if we don't have the node's
    /// sender anymore.
    pub fn send_want_request(
        &self,
        remote: &NodePublicKey,
        remote_index: RemoteInternedId,
    ) -> bool {
        log::trace!("sending want request to {remote} for {remote_index}");
        let Some(info) = self.peers.get(remote) else {
            return false;
        };
        let sender = info.sender.clone();
        let frame = Frame::Want(Want {
            interned_id: remote_index,
        });
        tokio::spawn(async move {
            sender.send(frame).await;
        });
        true
    }

    /// Advertise a given digest with the given assigned interned id to all the connected
    /// peers that we have. Obviously, we don't do it for the nodes that we already know
    /// have this message.
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
                //
                // struct Envelop<T>(Vec<u8>, PhantomData<T>);
                // let e = Envelop::new(msg);
                // ...
                // send_envelop(e)
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

        // TODO(qti3e): Find a good spawn threshold.
        if state.len() >= 100 {
            tokio::spawn(async move {
                for (pk, info) in state {
                    closure(&stats, (pk, info));
                }
            });
        } else {
            for (pk, info) in state {
                closure(&stats, (pk, info));
            }
        }
    }

    pub fn handle_new_connection(
        &mut self,
        origin: ConnectionOrigin,
        index: NodeIndex,
        (sender, receiver): (S, R),
    ) {
        enum DisputeResolveResponse {
            AcceptNewConnection,
            RejectNewConnection,
        }

        // If a connection already exists with the peer figures our which one we should
        // keep.
        fn resolve_dispute(
            prev_origin: ConnectionOrigin,
            origin: ConnectionOrigin,
            us: NodeIndex,
            index: NodeIndex,
        ) -> DisputeResolveResponse {
            // If the direction of the call is the same as before accept it,
            // it might be a connection refresh or something.
            if origin == prev_origin {
                return DisputeResolveResponse::AcceptNewConnection;
            }

            match (origin, index.cmp(&us)) {
                // For two cases below we are the one who is making the new call. So the new
                // connection is from us. Lower index wins, so we have to reject our own connection
                // if the peer has lower index in order to keep the other peers connection to us
                // alive.
                (ConnectionOrigin::Us, std::cmp::Ordering::Less) => {
                    DisputeResolveResponse::RejectNewConnection
                },
                (ConnectionOrigin::Us, _) => DisputeResolveResponse::AcceptNewConnection,
                // The peer is calling us, the connection we currently have was one made from us ->
                // remote. We have to accept remote's call if they have a lower index than us.
                (ConnectionOrigin::Remote, std::cmp::Ordering::Less) => {
                    DisputeResolveResponse::AcceptNewConnection
                },
                (ConnectionOrigin::Remote, _) => DisputeResolveResponse::RejectNewConnection,
            }
        }

        assert_ne!(index, self.us); // we shouldn't be calling ourselves.
        let (sender, receiver) = create_pair(sender, receiver);
        let pk = *sender.pk();
        let mut has = None;

        if let Some(info) = self.peers.get(&pk) {
            debug_assert_eq!(index, info.index);

            match resolve_dispute(info.origin, origin, self.us, index) {
                DisputeResolveResponse::AcceptNewConnection => {
                    let info = self.peers.remove(&pk).unwrap();
                    // Keep the info we have about this peer.
                    has = Some(info.has);
                    // Drop the sender in 5 seconds to close the connection. Since we are using
                    // `PairedSender/Receiver` this will result in the drop of receiver as well.
                    let sender = info.sender;
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        log::trace!("dropping {sender:?}");
                        drop(sender);
                    });
                },
                DisputeResolveResponse::RejectNewConnection => {
                    // For 5 second process messages coming from this new receiver anyway because
                    // the remote might be sending valuable information.
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        log::trace!("dropping {sender:?}");
                        drop(sender);
                    });
                    self.incoming_messages.push(receiver);

                    // Exit early and don't store this new connection.
                    return;
                },
            }
        }

        let info = Peer {
            index,
            sender,
            origin: ConnectionOrigin::Remote,
            status: ConnectionStatus::Open,
            has: has.unwrap_or_default(),
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
            pinned: Default::default(),
            stats: Default::default(),
            peers: Default::default(),
            incoming_messages: Default::default(),
        }
    }
}
