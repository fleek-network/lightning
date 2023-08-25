use fleek_crypto::NodePublicKey;
use fxhash::FxHashMap;

use lightning_interfaces::{SenderInterface};

use crate::frame::{Digest, Frame};

/// This struct is responsible for holding the state of the current peers
/// that we are connected to.
pub struct Peers<S: SenderInterface<Frame>> {
    map: FxHashMap<NodePublicKey, Peer<S>>
}

struct Peer<S> {
    sender: S,
}

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

impl<S: SenderInterface<Frame>> Peers<S> {
    /// Advertise the given digest to every peer. This method does not perform
    /// the advertisements with the peers if we know they already have the digest.
    ///
    /// That is when they have previously advertised the same digest to us.
    pub fn advertise(&self, _digest: Digest) {}
}

impl<S: SenderInterface<Frame>> Default for Peers<S> {
    fn default() -> Self {
        Self { map: Default::default() }
    }
}
