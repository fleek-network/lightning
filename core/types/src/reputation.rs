//! Types used by the reputation interface.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Contains the peer measurements that node A has about node B, that
/// will be taken into account when computing B's reputation score.
#[derive(Clone, Debug, Eq, PartialEq, Default, Hash, Serialize, Deserialize)]
pub struct ReputationMeasurements {
    pub latency: Option<Duration>,
    pub interactions: Option<i64>,
    pub inbound_bandwidth: Option<u128>,
    pub outbound_bandwidth: Option<u128>,
    pub bytes_received: Option<u128>,
    pub bytes_sent: Option<u128>,
    pub uptime: Option<u8>,
    pub hops: Option<u8>,
}
