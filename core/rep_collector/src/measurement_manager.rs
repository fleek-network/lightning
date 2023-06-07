use std::collections::HashMap;

use fleek_crypto::NodePublicKey;

/// Manages the measurements for all the peers.
pub struct MeasurementManager {
    _peers: HashMap<NodePublicKey, Measurements>,
}

impl MeasurementManager {
    pub fn new() -> Self {
        Self {
            _peers: HashMap::new(),
        }
    }
}

/// Holds all the current measurements for a particular peer.
struct Measurements {}
