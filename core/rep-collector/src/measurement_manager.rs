use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc, time::Duration};

use draco_interfaces::{types::ReputationMeasurements, Weight};
use draco_reputation::statistics::min_max_normalize;
use fleek_crypto::NodePublicKey;
use lru::LruCache;

/// Maximum capacity for the lru cache that stores the peer measurements.
const MAX_CAPACITY: usize = 200;

/// The rep score of a node is the exponentially weighted moving average of its rep scores over the
/// past epochs.
/// For example, `REP_EWMA_WEIGHT=0.7` means that 70% of the current rep score is based on past
/// epochs and 30% is based on the current epoch.
const REP_EWMA_WEIGHT: f64 = 0.7;

/// Manages the measurements for all the peers.
pub struct MeasurementManager {
    peers: LruCache<NodePublicKey, MeasurementStore>,
    summary_stats: SummaryStatistics,
    local_reputation: Arc<scc::HashMap<NodePublicKey, u128>>,
}

impl MeasurementManager {
    pub fn new() -> Self {
        Self {
            peers: LruCache::new(NonZeroUsize::new(MAX_CAPACITY).unwrap()),
            summary_stats: SummaryStatistics::default(),
            local_reputation: Arc::new(scc::HashMap::new()),
        }
    }

    pub fn clear_measurements(&mut self) {
        self.peers.clear();
    }

    pub fn get_measurements(&self) -> BTreeMap<NodePublicKey, ReputationMeasurements> {
        self.peers
            .iter()
            .map(|(peer, measurement_store)| (*peer, measurement_store.into()))
            .collect()
    }

    pub fn get_local_reputation_ref(&self) -> Arc<scc::HashMap<NodePublicKey, u128>> {
        self.local_reputation.clone()
    }

    pub fn report_sat(&mut self, peer: NodePublicKey, weight: Weight) {
        self.peers
            .get_or_insert_mut(peer, MeasurementStore::default)
            .register_interaction(true, weight);
        let value = self.peers.get(&peer).unwrap().interactions.get().unwrap();
        self.summary_stats.update_interactions(value);
        self.update_local_reputation_score(peer);
    }

    pub fn report_unsat(&mut self, peer: NodePublicKey, weight: Weight) {
        self.peers
            .get_or_insert_mut(peer, MeasurementStore::default)
            .register_interaction(false, weight);
        let value = self.peers.get(&peer).unwrap().interactions.get().unwrap();
        self.summary_stats.update_interactions(value);
        self.update_local_reputation_score(peer);
    }

    pub fn report_latency(&mut self, peer: NodePublicKey, latency: Duration) {
        self.peers
            .get_or_insert_mut(peer, MeasurementStore::default)
            .register_latency(latency);
        let value = self.peers.get(&peer).unwrap().latency.get().unwrap();
        self.summary_stats.update_latency(value);
        self.update_local_reputation_score(peer);
    }

    pub fn report_bytes_received(
        &mut self,
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        let measurements = self
            .peers
            .get_or_insert_mut(peer, MeasurementStore::default);
        measurements.register_bytes_received(bytes);
        let value = measurements.bytes_received.get();
        self.summary_stats.update_bytes_received(value);
        if let Some(duration) = duration {
            measurements.register_outbound_bandwidth(bytes, duration);
            let value = measurements.outbound_bandwidth.get().unwrap();
            self.summary_stats.update_outbound_bandwidth(value);
        }
        self.update_local_reputation_score(peer);
    }

    pub fn report_bytes_sent(
        &mut self,
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        let measurements = self
            .peers
            .get_or_insert_mut(peer, MeasurementStore::default);
        measurements.register_bytes_sent(bytes);
        let value = measurements.bytes_sent.get();
        self.summary_stats.update_bytes_sent(value);
        if let Some(duration) = duration {
            measurements.register_inbound_bandwidth(bytes, duration);
            let value = measurements.inbound_bandwidth.get().unwrap();
            self.summary_stats.update_inbound_bandwidth(value);
        }
        self.update_local_reputation_score(peer);
    }

    pub fn report_hops(&mut self, peer: NodePublicKey, hops: u8) {
        self.peers
            .get_or_insert_mut(peer, MeasurementStore::default)
            .register_hops(hops);
        let value = self.peers.get(&peer).unwrap().hops.get().unwrap();
        self.summary_stats.update_hops(value);
    }

    fn update_local_reputation_score(&mut self, peer: NodePublicKey) {
        if let Some(measurements) = self.peers.get(&peer) {
            let measurements: ReputationMeasurements = measurements.into();
            let norm_measurements = NormalizedMeasurements::new(measurements, &self.summary_stats);
            let mut score = 0.0;
            let mut count = 0;
            if let Some(latency) = norm_measurements.latency {
                score += 1.0 - latency;
                count += 1;
            }
            if let Some(interactions) = norm_measurements.interactions {
                score += interactions;
                count += 1;
            }
            if let Some(inbound_bandwidth) = norm_measurements.inbound_bandwidth {
                score += inbound_bandwidth;
                count += 1;
            }
            if let Some(outbound_bandwidth) = norm_measurements.outbound_bandwidth {
                score += outbound_bandwidth;
                count += 1;
            }
            if let Some(bytes_received) = norm_measurements.bytes_received {
                score += bytes_received;
                count += 1;
            }
            if let Some(bytes_sent) = norm_measurements.bytes_sent {
                score += bytes_sent;
                count += 1;
            }
            score /= count as f64;
            let score = score * 100.0;
            self.local_reputation
                .entry(peer)
                .and_modify(|s| {
                    *s = (*s as f64 * REP_EWMA_WEIGHT + (1.0 - REP_EWMA_WEIGHT) * score) as u128
                })
                .or_insert(score as u128);
        }
    }
}

/// Holds all the current measurements for a particular peer.
#[derive(Clone)]
struct MeasurementStore {
    latency: Latency,
    interactions: Interactions,
    inbound_bandwidth: Bandwidth,
    outbound_bandwidth: Bandwidth,
    bytes_received: BytesTransferred,
    bytes_sent: BytesTransferred,
    hops: Hops,
}

impl Default for MeasurementStore {
    fn default() -> Self {
        Self {
            latency: Latency::new(),
            interactions: Interactions::new(),
            inbound_bandwidth: Bandwidth::new(),
            outbound_bandwidth: Bandwidth::new(),
            bytes_received: BytesTransferred::new(),
            bytes_sent: BytesTransferred::new(),
            hops: Hops::new(),
        }
    }
}

impl MeasurementStore {
    fn register_latency(&mut self, latency: Duration) {
        self.latency.register_latency(latency);
    }

    fn register_interaction(&mut self, sat: bool, weight: Weight) {
        self.interactions.register_interaction(sat, weight);
    }

    fn register_inbound_bandwidth(&mut self, bytes: u64, duration: Duration) {
        self.inbound_bandwidth
            .register_bytes_transferred(bytes, duration);
    }

    fn register_outbound_bandwidth(&mut self, bytes: u64, duration: Duration) {
        self.outbound_bandwidth
            .register_bytes_transferred(bytes, duration);
    }

    fn register_bytes_received(&mut self, bytes: u64) {
        self.bytes_received.register_bytes_transferred(bytes);
    }

    fn register_bytes_sent(&mut self, bytes: u64) {
        self.bytes_sent.register_bytes_transferred(bytes);
    }

    fn register_hops(&mut self, hops: u8) {
        self.hops.register_hops(hops);
    }
}

#[derive(Clone)]
struct Latency {
    sum: Duration,
    count: u32,
}

impl Latency {
    fn new() -> Self {
        Self {
            sum: Duration::from_millis(0),
            count: 0,
        }
    }

    fn register_latency(&mut self, latency: Duration) {
        if latency.as_millis() > 0 {
            self.sum += latency;
            self.count += 1;
        }
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<Duration> {
        if self.count > 0 {
            Some(self.sum / self.count)
        } else {
            None
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
struct Interactions {
    sum: Option<i64>,
}

impl Interactions {
    fn new() -> Self {
        Self { sum: None }
    }

    fn register_interaction(&mut self, sat: bool, weight: Weight) {
        if sat {
            self.sum = Some(self.sum.unwrap_or(0) + Interactions::get_weight(weight));
        } else {
            self.sum = Some(self.sum.unwrap_or(0) - Interactions::get_weight(weight));
        }
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<i64> {
        self.sum
    }

    fn get_weight(weight: Weight) -> i64 {
        match weight {
            Weight::Weak => 1,
            Weight::Strong => 5,
            Weight::VeryStrong => 10,
            Weight::Provable => 20,
        }
    }
}

#[derive(Clone)]
struct Bandwidth {
    bytes_per_ms_sum: f64,
    count: u64,
}

impl Bandwidth {
    fn new() -> Self {
        Self {
            bytes_per_ms_sum: 0.0,
            count: 0,
        }
    }

    fn register_bytes_transferred(&mut self, bytes: u64, duration: Duration) {
        let bytes_per_ms = bytes as f64 / duration.as_millis() as f64;
        self.bytes_per_ms_sum += bytes_per_ms;
        self.count += 1;
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<u128> {
        if self.count > 0 {
            Some((self.bytes_per_ms_sum / (self.count as f64)) as u128)
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct BytesTransferred {
    bytes: u128,
}

impl BytesTransferred {
    fn new() -> Self {
        Self { bytes: 0 }
    }

    fn register_bytes_transferred(&mut self, bytes: u64) {
        self.bytes += bytes as u128;
    }

    #[allow(dead_code)]
    fn get(&self) -> u128 {
        self.bytes
    }
}

#[derive(Clone)]
struct Hops {
    hops: Option<u8>,
}

impl Hops {
    fn new() -> Self {
        Self { hops: None }
    }

    fn register_hops(&mut self, hops: u8) {
        self.hops = Some(hops)
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<u8> {
        self.hops
    }
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct SummaryStatistics {
    min: ReputationMeasurements,
    max: ReputationMeasurements,
}

#[allow(dead_code)]
impl SummaryStatistics {
    fn update_latency(&mut self, value: Duration) {
        self.min.latency = Self::update(self.min.latency, value, &std::cmp::min);
        self.max.latency = Self::update(self.max.latency, value, &std::cmp::max);
    }

    fn update_interactions(&mut self, value: i64) {
        self.min.interactions = Self::update(self.min.interactions, value, &std::cmp::min);
        self.max.interactions = Self::update(self.max.interactions, value, &std::cmp::max);
    }

    fn update_inbound_bandwidth(&mut self, value: u128) {
        self.min.inbound_bandwidth =
            Self::update(self.min.inbound_bandwidth, value, &std::cmp::min);
        self.max.inbound_bandwidth =
            Self::update(self.max.inbound_bandwidth, value, &std::cmp::max);
    }

    fn update_outbound_bandwidth(&mut self, value: u128) {
        self.min.outbound_bandwidth =
            Self::update(self.min.outbound_bandwidth, value, &std::cmp::min);
        self.max.outbound_bandwidth =
            Self::update(self.max.outbound_bandwidth, value, &std::cmp::max);
    }

    fn update_bytes_received(&mut self, value: u128) {
        self.min.bytes_received = Self::update(self.min.bytes_received, value, &std::cmp::min);
        self.max.bytes_received = Self::update(self.max.bytes_received, value, &std::cmp::max);
    }

    fn update_bytes_sent(&mut self, value: u128) {
        self.min.bytes_sent = Self::update(self.min.bytes_sent, value, &std::cmp::min);
        self.max.bytes_sent = Self::update(self.max.bytes_sent, value, &std::cmp::max);
    }

    fn update_hops(&mut self, value: u8) {
        self.min.hops = Self::update(self.min.hops, value, &std::cmp::min);
        self.max.hops = Self::update(self.max.hops, value, &std::cmp::max);
    }

    fn update<T: PartialOrd + Copy>(
        summary_val: Option<T>,
        new_value: T,
        cmp: &dyn Fn(T, T) -> T,
    ) -> Option<T> {
        match summary_val {
            Some(summary_val) => Some(cmp(summary_val, new_value)),
            None => Some(new_value),
        }
    }
}

impl From<&MeasurementStore> for ReputationMeasurements {
    fn from(value: &MeasurementStore) -> Self {
        Self {
            latency: value.latency.get(),
            interactions: value.interactions.get(),
            inbound_bandwidth: value.inbound_bandwidth.get(),
            outbound_bandwidth: value.outbound_bandwidth.get(),
            bytes_received: Some(value.bytes_received.get()),
            bytes_sent: Some(value.bytes_sent.get()),
            hops: value.hops.get(),
        }
    }
}

#[derive(Debug)]
struct NormalizedMeasurements {
    latency: Option<f64>,
    interactions: Option<f64>,
    inbound_bandwidth: Option<f64>,
    outbound_bandwidth: Option<f64>,
    bytes_received: Option<f64>,
    bytes_sent: Option<f64>,
}

impl NormalizedMeasurements {
    fn new(values: ReputationMeasurements, summary_stats: &SummaryStatistics) -> Self {
        let latency = if let (Some(min_val), Some(max_val)) =
            (summary_stats.min.latency, summary_stats.max.latency)
        {
            values.latency.map(|x| {
                min_max_normalize(
                    x.as_millis() as f64,
                    min_val.as_millis() as f64,
                    max_val.as_millis() as f64,
                )
            })
        } else {
            None
        };
        let interactions = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min.interactions,
            summary_stats.max.interactions,
        ) {
            values
                .interactions
                .map(|x| min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let inbound_bandwidth = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min.inbound_bandwidth,
            summary_stats.max.inbound_bandwidth,
        ) {
            values
                .inbound_bandwidth
                .map(|x| min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let outbound_bandwidth = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min.outbound_bandwidth,
            summary_stats.max.outbound_bandwidth,
        ) {
            values
                .outbound_bandwidth
                .map(|x| min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let bytes_received = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min.bytes_received,
            summary_stats.max.bytes_received,
        ) {
            values
                .bytes_received
                .map(|x| min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let bytes_sent = if let (Some(min_val), Some(max_val)) =
            (summary_stats.min.bytes_sent, summary_stats.max.bytes_sent)
        {
            values
                .bytes_sent
                .map(|x| min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        Self {
            latency,
            interactions,
            inbound_bandwidth,
            outbound_bandwidth,
            bytes_received,
            bytes_sent,
        }
    }
}

#[cfg(test)]
mod tests {
    use draco_interfaces::Weight;
    use fleek_crypto::NodePublicKey;

    use super::*;

    #[test]
    fn test_report_sat() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_sat(peer, Weight::Weak);
        let measurements = manager.peers.get(&peer).unwrap();
        assert_eq!(
            measurements.interactions.get().unwrap(),
            Interactions::get_weight(Weight::Weak)
        );
    }

    #[test]
    fn test_report_unsat() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_unsat(peer, Weight::Weak);
        let measurements = manager.peers.get(&peer).unwrap();
        assert_eq!(
            measurements.interactions.get().unwrap(),
            -Interactions::get_weight(Weight::Weak)
        );
    }

    #[test]
    fn test_report_sat_unsat() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_sat(peer, Weight::Weak);
        manager.report_unsat(peer, Weight::Weak);
        let measurements = manager.peers.get(&peer).unwrap();
        assert_eq!(measurements.interactions.get().unwrap(), 0);
    }

    #[test]
    fn test_report_latency() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_latency(peer, Duration::from_millis(200));
        manager.report_latency(peer, Duration::from_millis(100));
        let measurements = manager.peers.get(&peer).unwrap();
        assert_eq!(
            measurements.latency.get().unwrap(),
            Duration::from_millis(150)
        );
    }

    #[test]
    fn test_report_bytes_received() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_bytes_received(peer, 1024, Some(Duration::from_millis(200)));
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.bytes_received.get(), 1024);
        assert_eq!(measurements.outbound_bandwidth.get().unwrap(), 5);
    }

    #[test]
    fn test_report_bytes_sent() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_bytes_sent(peer, 1024, Some(Duration::from_millis(200)));
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.bytes_sent.get(), 1024);
        assert_eq!(measurements.inbound_bandwidth.get().unwrap(), 5);
    }

    #[test]
    fn test_report_hops() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_hops(peer, 10);
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.hops.get().unwrap(), 10);
    }

    #[test]
    fn test_lateny_min_max() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_latency(peer, Duration::from_millis(200));
        let peer = NodePublicKey([1; 96]);
        manager.report_latency(peer, Duration::from_millis(100));
        assert_eq!(
            manager.summary_stats.min.latency.unwrap(),
            Duration::from_millis(100)
        );
        assert_eq!(
            manager.summary_stats.max.latency.unwrap(),
            Duration::from_millis(200)
        );
    }

    #[test]
    fn test_interactions_min_max() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_sat(peer, Weight::Weak);
        let peer = NodePublicKey([1; 96]);
        manager.report_sat(peer, Weight::Strong);
        assert_eq!(
            manager.summary_stats.min.interactions.unwrap(),
            Interactions::get_weight(Weight::Weak)
        );
        assert_eq!(
            manager.summary_stats.max.interactions.unwrap(),
            Interactions::get_weight(Weight::Strong)
        );
    }

    #[test]
    fn test_bytes_received_min_max() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_bytes_received(peer, 1000, Some(Duration::from_millis(200)));
        let peer = NodePublicKey([1; 96]);
        manager.report_bytes_received(peer, 2000, Some(Duration::from_millis(100)));

        assert_eq!(manager.summary_stats.min.bytes_received.unwrap(), 1000);
        assert_eq!(manager.summary_stats.min.outbound_bandwidth.unwrap(), 5);

        assert_eq!(manager.summary_stats.max.bytes_received.unwrap(), 2000);
        assert_eq!(manager.summary_stats.max.outbound_bandwidth.unwrap(), 20);
    }

    #[test]
    fn test_bytes_sent_min_max() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_bytes_sent(peer, 1000, Some(Duration::from_millis(200)));
        let peer = NodePublicKey([1; 96]);
        manager.report_bytes_sent(peer, 2000, Some(Duration::from_millis(100)));

        assert_eq!(manager.summary_stats.min.bytes_sent.unwrap(), 1000);
        assert_eq!(manager.summary_stats.min.inbound_bandwidth.unwrap(), 5);

        assert_eq!(manager.summary_stats.max.bytes_sent.unwrap(), 2000);
        assert_eq!(manager.summary_stats.max.inbound_bandwidth.unwrap(), 20);
    }

    #[test]
    fn test_hops_min_max() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_hops(peer, 10);
        let peer = NodePublicKey([1; 96]);
        manager.report_hops(peer, 20);

        assert_eq!(manager.summary_stats.min.hops.unwrap(), 10);
        assert_eq!(manager.summary_stats.max.hops.unwrap(), 20);
    }

    #[test]
    fn test_get_local_reputation_ref() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_sat(peer, Weight::Weak);
        let reputation_map = manager.get_local_reputation_ref();
        assert!(reputation_map.contains(&peer));
    }

    #[test]
    fn test_get_measurements_contains() {
        let mut manager = MeasurementManager::new();
        let peer1 = NodePublicKey([0; 96]);
        manager.report_sat(peer1, Weight::Weak);
        let peer2 = NodePublicKey([1; 96]);
        manager.report_sat(peer2, Weight::Weak);
        let peer_measurements = manager.get_measurements();
        assert!(peer_measurements.contains_key(&peer1));
        assert!(peer_measurements.contains_key(&peer2));
    }

    #[test]
    fn test_get_measurements_equals() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_sat(peer, Weight::Weak);
        manager.report_latency(peer, Duration::from_millis(200));
        let peer_measurements = manager.get_measurements();
        let measurements = peer_measurements.get(&peer).unwrap();
        assert_eq!(
            measurements.interactions.unwrap(),
            Interactions::get_weight(Weight::Weak)
        );
        assert_eq!(measurements.latency.unwrap(), Duration::from_millis(200));
    }

    #[test]
    fn test_clear_measurements() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_sat(peer, Weight::Weak);
        let peer_measurements = manager.get_measurements();
        assert!(peer_measurements.contains_key(&peer));
        manager.clear_measurements();
        let peer_measurements = manager.get_measurements();
        assert!(!peer_measurements.contains_key(&peer));
    }
}
