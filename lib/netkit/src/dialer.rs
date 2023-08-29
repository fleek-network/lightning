use std::collections::HashSet;
use std::future::Future;

use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use quinn::{Connection, Endpoint};

use crate::endpoint::NodeAddress;

/// Connects to peers.
pub struct Dialer {
    /// QUIC endpoint.
    endpoint: Endpoint,
    /// Ongoing dialing tasks.
    ongoing: FuturesUnordered<Box<dyn Future<Output = (NodePublicKey, Connection)>>>,
    /// Pending dialing tasks.
    pending: HashSet<NodeAddress>,
}
