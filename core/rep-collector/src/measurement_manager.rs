use std::{num::NonZeroUsize, sync::Arc, time::Duration};

use draco_interfaces::Weight;
use fleek_crypto::NodePublicKey;
use lru::LruCache;

const MAX_CAPACITY: usize = 200;

#[allow(dead_code)]
/// Manages the measurements for all the peers.
pub struct MeasurementManager {
    peers: LruCache<NodePublicKey, Measurements>,
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

    pub fn get_local_reputation_ref(&self) -> Arc<scc::HashMap<NodePublicKey, u128>> {
        self.local_reputation.clone()
    }

    pub fn report_sat(&mut self, peer: NodePublicKey, weight: Weight) {
        self.peers
            .get_or_insert_mut(peer, Measurements::default)
            .register_interaction(true, weight);
        let value = self.peers.get(&peer).unwrap().interactions.get().unwrap();
        self.summary_stats.update_interactions(value);
    }

    pub fn report_unsat(&mut self, peer: NodePublicKey, weight: Weight) {
        self.peers
            .get_or_insert_mut(peer, Measurements::default)
            .register_interaction(false, weight);
        let value = self.peers.get(&peer).unwrap().interactions.get().unwrap();
        self.summary_stats.update_interactions(value);
    }

    pub fn report_latency(&mut self, peer: NodePublicKey, latency: Duration) {
        self.peers
            .get_or_insert_mut(peer, Measurements::default)
            .register_latency(latency);
        let value = self.peers.get(&peer).unwrap().latency.get().unwrap();
        self.summary_stats.update_latency(value);
    }

    pub fn report_bytes_received(
        &mut self,
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        let measurements = self.peers.get_or_insert_mut(peer, Measurements::default);
        measurements.register_bytes_received(bytes);
        let value = measurements.bytes_received.get();
        self.summary_stats.update_bytes_received(value);
        if let Some(duration) = duration {
            measurements.register_outbound_bandwidth(bytes, duration);
            let value = measurements.outbound_bandwidth.get().unwrap();
            self.summary_stats.update_outbound_bandwidth(value);
        }
    }

    pub fn report_bytes_sent(
        &mut self,
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        let measurements = self.peers.get_or_insert_mut(peer, Measurements::default);
        measurements.register_bytes_sent(bytes);
        let value = measurements.bytes_sent.get();
        self.summary_stats.update_bytes_sent(value);
        if let Some(duration) = duration {
            measurements.register_inbound_bandwidth(bytes, duration);
            let value = measurements.inbound_bandwidth.get().unwrap();
            self.summary_stats.update_inbound_bandwidth(value);
        }
    }

    pub fn report_hops(&mut self, peer: NodePublicKey, hops: u8) {
        self.peers
            .get_or_insert_mut(peer, Measurements::default)
            .register_hops(hops);
        let value = self.peers.get(&peer).unwrap().hops.get().unwrap();
        self.summary_stats.update_hops(value);
    }
}

/// Holds all the current measurements for a particular peer.
#[derive(Clone)]
struct Measurements {
    latency: Latency,
    interactions: Interactions,
    inbound_bandwidth: Bandwidth,
    outbound_bandwidth: Bandwidth,
    bytes_received: BytesTransferred,
    bytes_sent: BytesTransferred,
    hops: Hops,
}

impl Default for Measurements {
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

impl Measurements {
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
    fn get(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.bytes_per_ms_sum / (self.count as f64))
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
    min: Values,
    max: Values,
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

    fn update_inbound_bandwidth(&mut self, value: f64) {
        self.min.inbound_bandwidth = Self::update(self.min.inbound_bandwidth, value, &f64::min);
        self.max.inbound_bandwidth = Self::update(self.max.inbound_bandwidth, value, &f64::max);
    }

    fn update_outbound_bandwidth(&mut self, value: f64) {
        self.min.outbound_bandwidth = Self::update(self.min.outbound_bandwidth, value, &f64::min);
        self.max.outbound_bandwidth = Self::update(self.max.outbound_bandwidth, value, &f64::max);
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
        min_val: Option<T>,
        value: T,
        cmp: &dyn Fn(T, T) -> T,
    ) -> Option<T> {
        match min_val {
            Some(min_val) => Some(cmp(min_val, value)),
            None => Some(value),
        }
    }
}

#[derive(Debug, Default)]
struct Values {
    latency: Option<Duration>,
    interactions: Option<i64>,
    inbound_bandwidth: Option<f64>,
    outbound_bandwidth: Option<f64>,
    bytes_received: Option<u128>,
    bytes_sent: Option<u128>,
    hops: Option<u8>,
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
        assert_eq!(measurements.outbound_bandwidth.get().unwrap(), 5.12);
    }

    #[test]
    fn test_report_bytes_sent() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_bytes_sent(peer, 1024, Some(Duration::from_millis(200)));
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.bytes_sent.get(), 1024);
        assert_eq!(measurements.inbound_bandwidth.get().unwrap(), 5.12);
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
        assert_eq!(manager.summary_stats.min.outbound_bandwidth.unwrap(), 5.0);

        assert_eq!(manager.summary_stats.max.bytes_received.unwrap(), 2000);
        assert_eq!(manager.summary_stats.max.outbound_bandwidth.unwrap(), 20.0);
    }

    #[test]
    fn test_bytes_sent_min_max() {
        let mut manager = MeasurementManager::new();
        let peer = NodePublicKey([0; 96]);
        manager.report_bytes_sent(peer, 1000, Some(Duration::from_millis(200)));
        let peer = NodePublicKey([1; 96]);
        manager.report_bytes_sent(peer, 2000, Some(Duration::from_millis(100)));

        assert_eq!(manager.summary_stats.min.bytes_sent.unwrap(), 1000);
        assert_eq!(manager.summary_stats.min.inbound_bandwidth.unwrap(), 5.0);

        assert_eq!(manager.summary_stats.max.bytes_sent.unwrap(), 2000);
        assert_eq!(manager.summary_stats.max.inbound_bandwidth.unwrap(), 20.0);
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
}
