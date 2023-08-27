use fleek_crypto::NodePublicKey;
use fxhash::{FxHashMap, FxHashSet};

use crate::Digest;

/// Responsible for keeping track of the pending want requests we have
/// out there.
///
/// The desired functionality is that we need to store an outgoing
/// want request here.
pub struct PendingStore {
    /// Map each pending digest to a set of nodes that we know have this digest.
    pending_map: FxHashMap<Digest, FxHashSet<NodePublicKey>>,
}

impl PendingStore {
    pub fn insert() {}
}
