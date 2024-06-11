use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use hp_fixed::signed::HpFixed;
use lightning_interfaces::types::{NodeIndex, ReputationMeasurements, PRECISION};
use lightning_interfaces::Weight;
use lightning_reputation::statistics::try_min_max_normalize;
use lru::LruCache;

/// Maximum capacity for the lru cache that stores the peer measurements.
const MAX_CAPACITY: usize = 200;

/// The rep score of a node is the exponentially weighted moving average of its rep scores over the
/// past epochs.
/// For example, `REP_EWMA_WEIGHT=0.7` means that 70% of the current rep score is based on past
/// epochs and 30% is based on the current epoch.
const REP_EWMA_WEIGHT: f64 = 0.7;

/// The minimum number of pings that must be recorded for a peer, in order to report uptime
/// measurements for that peer.
#[cfg(not(debug_assertions))]
const MIN_NUM_PINGS: usize = 3;
#[cfg(debug_assertions)]
const MIN_NUM_PINGS: usize = 1;

/// Manages the measurements for all the peers.
pub struct MeasurementManager {
    peers: LruCache<NodeIndex, MeasurementStore>,
    summary_stats: SummaryStatistics,
    local_reputation: Arc<scc::HashMap<NodeIndex, u8>>,
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
        self.summary_stats.clear();
    }

    pub fn get_measurements(&self) -> BTreeMap<NodeIndex, ReputationMeasurements> {
        self.peers
            .iter()
            .map(|(peer, measurement_store)| (*peer, measurement_store.into()))
            .collect()
    }

    pub fn get_local_reputation_ref(&self) -> Arc<scc::HashMap<NodeIndex, u8>> {
        self.local_reputation.clone()
    }

    pub fn report_sat(&mut self, peer: NodeIndex, weight: Weight) {
        self.insert_if_not_exists(&peer);
        let (old_val, new_val) = self
            .peers
            .get_mut(&peer)
            .unwrap()
            .register_interaction(true, weight);
        self.summary_stats.remove_interactions(old_val);
        self.summary_stats.add_interactions(new_val);
        self.update_local_reputation_score(peer);
    }

    pub fn report_unsat(&mut self, peer: NodeIndex, weight: Weight) {
        self.insert_if_not_exists(&peer);
        let (old_val, new_val) = self
            .peers
            .get_mut(&peer)
            .unwrap()
            .register_interaction(false, weight);
        self.summary_stats.remove_interactions(old_val);
        self.summary_stats.add_interactions(new_val);
        self.update_local_reputation_score(peer);
    }

    pub fn report_latency(&mut self, peer: NodeIndex, latency: Duration) {
        self.insert_if_not_exists(&peer);
        let (old_val, new_val) = self.peers.get_mut(&peer).unwrap().register_latency(latency);
        self.summary_stats.remove_latency(old_val);
        self.summary_stats.add_latency(new_val);
        self.update_local_reputation_score(peer);
    }

    pub fn report_bytes_received(
        &mut self,
        peer: NodeIndex,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        self.insert_if_not_exists(&peer);
        let (old_val, new_val) = self
            .peers
            .get_mut(&peer)
            .unwrap()
            .register_bytes_received(bytes);
        self.summary_stats.remove_bytes_received(old_val);
        self.summary_stats.add_bytes_received(new_val);

        if let Some(duration) = duration {
            let (old_val, new_val) = self
                .peers
                .get_mut(&peer)
                .unwrap()
                .register_outbound_bandwidth(bytes, duration);
            self.summary_stats.remove_outbound_bandwidth(old_val);
            self.summary_stats.add_outbound_bandwidth(new_val);
        }
        self.update_local_reputation_score(peer);
    }

    pub fn report_bytes_sent(&mut self, peer: NodeIndex, bytes: u64, duration: Option<Duration>) {
        self.insert_if_not_exists(&peer);
        let (old_val, new_val) = self
            .peers
            .get_mut(&peer)
            .unwrap()
            .register_bytes_sent(bytes);
        self.summary_stats.remove_bytes_sent(old_val);
        self.summary_stats.add_bytes_sent(new_val);

        if let Some(duration) = duration {
            let (old_val, new_val) = self
                .peers
                .get_mut(&peer)
                .unwrap()
                .register_inbound_bandwidth(bytes, duration);
            self.summary_stats.remove_inbound_bandwidth(old_val);
            self.summary_stats.add_inbound_bandwidth(new_val);
        }
        self.update_local_reputation_score(peer);
    }

    pub fn report_ping(&mut self, peer: NodeIndex, responded: bool) {
        self.insert_if_not_exists(&peer);
        self.peers.get_mut(&peer).unwrap().register_ping(responded);
    }

    pub fn report_hops(&mut self, peer: NodeIndex, hops: u8) {
        self.insert_if_not_exists(&peer);
        let (old_val, new_val) = self.peers.get_mut(&peer).unwrap().register_hops(hops);
        self.summary_stats.remove_hops(old_val);
        self.summary_stats.add_hops(new_val);
    }

    fn insert_if_not_exists(&mut self, peer: &NodeIndex) {
        if !self.peers.contains(peer) {
            if let Some((_, measurements)) = self.peers.push(*peer, MeasurementStore::default()) {
                // If the insertion removed measurements from the lru cache, we have to update the
                // summary stats accordingly.
                self.summary_stats.remove(measurements);
            }
        }
    }

    fn update_local_reputation_score(&mut self, peer: NodeIndex) {
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
            if let Some(uptime) = norm_measurements.uptime {
                score += uptime;
                count += 1;
            }
            if count != 0 {
                score /= count as f64;
                score *= 100.0;
                self.local_reputation
                    .entry(peer)
                    .and_modify(|s| {
                        *s = (*s as f64 * REP_EWMA_WEIGHT + (1.0 - REP_EWMA_WEIGHT) * score) as u8
                    })
                    .or_insert(score as u8);
            }
        }
    }
}

/// Holds all the current measurements for a particular peer.
#[derive(Clone, Debug)]
struct MeasurementStore {
    latency: Latency,
    interactions: Interactions,
    inbound_bandwidth: Bandwidth,
    outbound_bandwidth: Bandwidth,
    bytes_received: BytesTransferred,
    bytes_sent: BytesTransferred,
    pings: Pings,
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
            pings: Pings::new(),
            hops: Hops::new(),
        }
    }
}

impl MeasurementStore {
    fn register_latency(&mut self, latency: Duration) -> (Option<Duration>, Option<Duration>) {
        self.latency.register_latency(latency)
    }

    fn register_interaction(&mut self, sat: bool, weight: Weight) -> (Option<i64>, Option<i64>) {
        self.interactions.register_interaction(sat, weight)
    }

    fn register_inbound_bandwidth(
        &mut self,
        bytes: u64,
        duration: Duration,
    ) -> (Option<u128>, Option<u128>) {
        self.inbound_bandwidth
            .register_bytes_transferred(bytes, duration)
    }

    fn register_outbound_bandwidth(
        &mut self,
        bytes: u64,
        duration: Duration,
    ) -> (Option<u128>, Option<u128>) {
        self.outbound_bandwidth
            .register_bytes_transferred(bytes, duration)
    }

    fn register_bytes_received(&mut self, bytes: u64) -> (u128, u128) {
        self.bytes_received.register_bytes_transferred(bytes)
    }

    fn register_bytes_sent(&mut self, bytes: u64) -> (u128, u128) {
        self.bytes_sent.register_bytes_transferred(bytes)
    }

    fn register_ping(&mut self, responded: bool) {
        self.pings.register_ping(responded);
    }

    fn register_hops(&mut self, hops: u8) -> (Option<u8>, Option<u8>) {
        self.hops.register_hops(hops)
    }
}

#[derive(Clone, Debug)]
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

    fn register_latency(&mut self, latency: Duration) -> (Option<Duration>, Option<Duration>) {
        let old_value = self.get();
        if latency.as_millis() > 0 {
            self.sum += latency;
            self.count += 1;
        }
        let new_value = self.get();
        (old_value, new_value)
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

#[derive(Clone, Debug)]
pub(crate) struct Interactions {
    sum: Option<i64>,
}

impl Interactions {
    fn new() -> Self {
        Self { sum: None }
    }

    fn register_interaction(&mut self, sat: bool, weight: Weight) -> (Option<i64>, Option<i64>) {
        let old_value = self.get();
        if sat {
            self.sum = Some(self.sum.unwrap_or(0) + Interactions::get_weight(weight));
        } else {
            self.sum = Some(self.sum.unwrap_or(0) - Interactions::get_weight(weight));
        }
        let new_value = self.get();
        (old_value, new_value)
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<i64> {
        self.sum
    }

    pub(crate) fn get_weight(weight: Weight) -> i64 {
        match weight {
            Weight::Weak => 1,
            Weight::Strong => 5,
            Weight::VeryStrong => 10,
            Weight::Provable => 20,
        }
    }
}

#[derive(Clone, Debug)]
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

    fn register_bytes_transferred(
        &mut self,
        bytes: u64,
        duration: Duration,
    ) -> (Option<u128>, Option<u128>) {
        let old_value = self.get();
        let bytes_per_ms = bytes as f64 / duration.as_millis() as f64;
        self.bytes_per_ms_sum += bytes_per_ms;
        self.count += 1;
        let new_value = self.get();
        (old_value, new_value)
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

#[derive(Clone, Debug)]
struct BytesTransferred {
    bytes: u128,
}

impl BytesTransferred {
    fn new() -> Self {
        Self { bytes: 0 }
    }

    fn register_bytes_transferred(&mut self, bytes: u64) -> (u128, u128) {
        let old_value = self.get();
        self.bytes += bytes as u128;
        let new_value = self.get();
        (old_value, new_value)
    }

    #[allow(dead_code)]
    fn get(&self) -> u128 {
        self.bytes
    }
}

#[derive(Clone, Debug)]
struct Pings {
    num_pings: usize,
    num_pings_responded: usize,
}

impl Pings {
    fn new() -> Self {
        Self {
            num_pings: 0,
            num_pings_responded: 0,
        }
    }

    fn register_ping(&mut self, responded: bool) {
        self.num_pings += 1;
        if responded {
            self.num_pings_responded += 1;
        }
    }

    fn get(&self) -> Option<HpFixed<PRECISION>> {
        if self.num_pings < MIN_NUM_PINGS {
            None
        } else {
            // ratio is in range [0, 1]
            let ratio = self.num_pings_responded as f64 / self.num_pings as f64;
            let ratio = HpFixed::from(ratio) * HpFixed::from(100);
            if ratio > HpFixed::from(100) {
                Some(HpFixed::from(100))
            } else {
                Some(ratio)
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Hops {
    hops: Option<u8>,
}

impl Hops {
    fn new() -> Self {
        Self { hops: None }
    }

    fn register_hops(&mut self, hops: u8) -> (Option<u8>, Option<u8>) {
        let old_value = self.get();
        self.hops = Some(hops);
        let new_value = self.get();
        (old_value, new_value)
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<u8> {
        self.hops
    }
}

#[derive(Debug, Default)]
struct SummaryStatistics {
    latency: BTreeMap<Duration, u32>,
    interactions: BTreeMap<i64, u32>,
    inbound_bandwidth: BTreeMap<u128, u32>,
    outbound_bandwidth: BTreeMap<u128, u32>,
    bytes_received: BTreeMap<u128, u32>,
    bytes_sent: BTreeMap<u128, u32>,
    hops: BTreeMap<u8, u32>,
}

impl SummaryStatistics {
    fn min_latency(&self) -> Option<Duration> {
        self.latency.first_key_value().map(|(v, _)| *v)
    }

    fn max_latency(&self) -> Option<Duration> {
        self.latency.last_key_value().map(|(v, _)| *v)
    }

    fn min_interactions(&self) -> Option<i64> {
        self.interactions.first_key_value().map(|(v, _)| *v)
    }

    fn max_interactions(&self) -> Option<i64> {
        self.interactions.last_key_value().map(|(v, _)| *v)
    }

    fn min_inbound_bandwidth(&self) -> Option<u128> {
        self.inbound_bandwidth.first_key_value().map(|(v, _)| *v)
    }

    fn max_inbound_bandwidth(&self) -> Option<u128> {
        self.inbound_bandwidth.last_key_value().map(|(v, _)| *v)
    }

    fn min_outbound_bandwidth(&self) -> Option<u128> {
        self.outbound_bandwidth.first_key_value().map(|(v, _)| *v)
    }

    fn max_outbound_bandwidth(&self) -> Option<u128> {
        self.outbound_bandwidth.last_key_value().map(|(v, _)| *v)
    }

    fn min_bytes_received(&self) -> Option<u128> {
        self.bytes_received.first_key_value().map(|(v, _)| *v)
    }

    fn max_bytes_received(&self) -> Option<u128> {
        self.bytes_received.last_key_value().map(|(v, _)| *v)
    }

    fn min_bytes_sent(&self) -> Option<u128> {
        self.bytes_sent.first_key_value().map(|(v, _)| *v)
    }

    fn max_bytes_sent(&self) -> Option<u128> {
        self.bytes_sent.last_key_value().map(|(v, _)| *v)
    }

    #[allow(dead_code)]
    fn min_hops(&self) -> Option<u8> {
        self.hops.first_key_value().map(|(v, _)| *v)
    }

    #[allow(dead_code)]
    fn max_hops(&self) -> Option<u8> {
        self.hops.last_key_value().map(|(v, _)| *v)
    }

    fn add_latency(&mut self, value: Option<Duration>) {
        if let Some(value) = value {
            self.latency
                .entry(value)
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
    }

    fn remove_latency(&mut self, value: Option<Duration>) {
        if let Some(value) = value {
            if let Some(count) = self.latency.get(&value) {
                if *count == 1 {
                    self.latency.remove(&value);
                } else {
                    self.latency.insert(value, count - 1);
                }
            }
        }
    }

    fn add_interactions(&mut self, value: Option<i64>) {
        if let Some(value) = value {
            self.interactions
                .entry(value)
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
    }

    fn remove_interactions(&mut self, value: Option<i64>) {
        if let Some(value) = value {
            if let Some(count) = self.interactions.get(&value) {
                if *count == 1 {
                    self.interactions.remove(&value);
                } else {
                    self.interactions.insert(value, count - 1);
                }
            }
        }
    }

    fn add_inbound_bandwidth(&mut self, value: Option<u128>) {
        if let Some(value) = value {
            self.inbound_bandwidth
                .entry(value)
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
    }

    fn remove_inbound_bandwidth(&mut self, value: Option<u128>) {
        if let Some(value) = value {
            if let Some(count) = self.inbound_bandwidth.get(&value) {
                if *count == 1 {
                    self.inbound_bandwidth.remove(&value);
                } else {
                    self.inbound_bandwidth.insert(value, count - 1);
                }
            }
        }
    }

    fn add_outbound_bandwidth(&mut self, value: Option<u128>) {
        if let Some(value) = value {
            self.outbound_bandwidth
                .entry(value)
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
    }

    fn remove_outbound_bandwidth(&mut self, value: Option<u128>) {
        if let Some(value) = value {
            if let Some(count) = self.outbound_bandwidth.get(&value) {
                if *count == 1 {
                    self.outbound_bandwidth.remove(&value);
                } else {
                    self.outbound_bandwidth.insert(value, count - 1);
                }
            }
        }
    }

    fn add_bytes_received(&mut self, value: u128) {
        self.bytes_received
            .entry(value)
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    fn remove_bytes_received(&mut self, value: u128) {
        if let Some(count) = self.bytes_received.get(&value) {
            if *count == 1 {
                self.bytes_received.remove(&value);
            } else {
                self.bytes_received.insert(value, count - 1);
            }
        }
    }

    fn add_bytes_sent(&mut self, value: u128) {
        self.bytes_sent
            .entry(value)
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    fn remove_bytes_sent(&mut self, value: u128) {
        if let Some(count) = self.bytes_sent.get(&value) {
            if *count == 1 {
                self.bytes_sent.remove(&value);
            } else {
                self.bytes_sent.insert(value, count - 1);
            }
        }
    }

    fn add_hops(&mut self, value: Option<u8>) {
        if let Some(value) = value {
            self.hops.entry(value).and_modify(|v| *v += 1).or_insert(1);
        }
    }

    fn remove_hops(&mut self, value: Option<u8>) {
        if let Some(value) = value {
            if let Some(count) = self.hops.get(&value) {
                if *count == 1 {
                    self.hops.remove(&value);
                } else {
                    self.hops.insert(value, count - 1);
                }
            }
        }
    }

    fn remove(&mut self, measurements: MeasurementStore) {
        self.remove_latency(measurements.latency.get());
        self.remove_interactions(measurements.interactions.get());
        self.remove_inbound_bandwidth(measurements.inbound_bandwidth.get());
        self.remove_outbound_bandwidth(measurements.outbound_bandwidth.get());
        self.remove_bytes_received(measurements.bytes_received.get());
        self.remove_bytes_sent(measurements.bytes_sent.get());
        self.remove_hops(measurements.hops.get());
    }

    fn clear(&mut self) {
        self.latency.clear();
        self.interactions.clear();
        self.inbound_bandwidth.clear();
        self.outbound_bandwidth.clear();
        self.bytes_received.clear();
        self.bytes_sent.clear();
        self.hops.clear();
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
            uptime: value.pings.get(),
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
    uptime: Option<f64>,
}

impl NormalizedMeasurements {
    fn new(values: ReputationMeasurements, summary_stats: &SummaryStatistics) -> Self {
        let latency = if let (Some(min_val), Some(max_val)) =
            (summary_stats.min_latency(), summary_stats.max_latency())
        {
            values.latency.and_then(|x| {
                try_min_max_normalize(
                    x.as_millis() as f64,
                    min_val.as_millis() as f64,
                    max_val.as_millis() as f64,
                )
            })
        } else {
            None
        };
        let interactions = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min_interactions(),
            summary_stats.max_interactions(),
        ) {
            values
                .interactions
                .and_then(|x| try_min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let inbound_bandwidth = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min_inbound_bandwidth(),
            summary_stats.max_inbound_bandwidth(),
        ) {
            values
                .inbound_bandwidth
                .and_then(|x| try_min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let outbound_bandwidth = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min_outbound_bandwidth(),
            summary_stats.max_outbound_bandwidth(),
        ) {
            values
                .outbound_bandwidth
                .and_then(|x| try_min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let bytes_received = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min_bytes_received(),
            summary_stats.max_bytes_received(),
        ) {
            values
                .bytes_received
                .and_then(|x| try_min_max_normalize(x as f64, min_val as f64, max_val as f64))
        } else {
            None
        };
        let bytes_sent = if let (Some(min_val), Some(max_val)) = (
            summary_stats.min_bytes_sent(),
            summary_stats.max_bytes_sent(),
        ) {
            values
                .bytes_sent
                .and_then(|x| try_min_max_normalize(x as f64, min_val as f64, max_val as f64))
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
            uptime: values
                .uptime
                .map(|x| x / HpFixed::from(100))
                .and_then(|x| f64::try_from(x).ok()),
        }
    }
}

#[cfg(test)]
mod tests {
    use lightning_interfaces::types::ReputationMeasurements;
    use lightning_interfaces::Weight;
    use lightning_test_utils::{random, reputation};
    use rand::Rng;

    use super::*;

    const PROB_MEASUREMENT_PRESENT: f64 = 0.1;

    fn generate_weighted_measurements(num_measurements: usize) -> Vec<ReputationMeasurements> {
        let mut rng = random::get_seedable_rng();

        let mut rep_measurements = Vec::with_capacity(num_measurements);
        for _ in 0..num_measurements {
            let measurements =
                reputation::generate_reputation_measurements(&mut rng, PROB_MEASUREMENT_PRESENT);
            rep_measurements.push(measurements);
        }
        rep_measurements
    }

    #[test]
    fn test_report_sat() {
        let mut manager = MeasurementManager::new();
        let peer = 0;
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
        let peer = 0;
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
        let peer = 0;
        manager.report_sat(peer, Weight::Weak);
        manager.report_unsat(peer, Weight::Weak);
        let measurements = manager.peers.get(&peer).unwrap();
        assert_eq!(measurements.interactions.get().unwrap(), 0);
    }

    #[test]
    fn test_report_latency() {
        let mut manager = MeasurementManager::new();
        let peer = 0;
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
        let peer = 0;
        manager.report_bytes_received(peer, 1024, Some(Duration::from_millis(200)));
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.bytes_received.get(), 1024);
        assert_eq!(measurements.outbound_bandwidth.get().unwrap(), 5);
    }

    #[test]
    fn test_report_bytes_sent() {
        let mut manager = MeasurementManager::new();
        let peer = 0;
        manager.report_bytes_sent(peer, 1024, Some(Duration::from_millis(200)));
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.bytes_sent.get(), 1024);
        assert_eq!(measurements.inbound_bandwidth.get().unwrap(), 5);
    }

    #[test]
    fn test_report_hops() {
        let mut manager = MeasurementManager::new();
        let peer = 0;
        manager.report_hops(peer, 10);
        let measurements = manager.peers.get(&peer).unwrap();

        assert_eq!(measurements.hops.get().unwrap(), 10);
    }

    #[test]
    fn test_report_ping() {
        let mut manager = MeasurementManager::new();
        let peer = 0;
        for _ in 0..MIN_NUM_PINGS {
            manager.report_ping(peer, false);
        }
        assert_eq!(
            manager.peers.get(&peer).unwrap().pings.get(),
            Some(HpFixed::from(0))
        );
        manager.report_ping(peer, true);
        assert_eq!(
            manager.peers.get(&peer).unwrap().pings.get(),
            // MIN_NUM_PINGS is defined differently based on cfg(debug_assertions), and tests in
            // garnix CI are run with release mode, so we support both cases.
            match MIN_NUM_PINGS {
                1 => Some(HpFixed::from(50)),
                3 => Some(HpFixed::from(25)),
                _ => panic!("MIN_NUM_PINGS is different than expected"),
            }
        );
        manager.report_ping(peer, true);
        assert_eq!(
            manager.peers.get(&peer).unwrap().pings.get(),
            // MIN_NUM_PINGS is defined differently based on cfg(debug_assertions), and tests in
            // garnix CI are run with release mode, so we support both cases.
            match MIN_NUM_PINGS {
                1 => Some(HpFixed::from(66.66666666666666)),
                3 => Some(HpFixed::from(40)),
                _ => panic!("MIN_NUM_PINGS is different than expected"),
            }
        );
        manager.report_ping(peer, false);
        assert_eq!(
            manager.peers.get(&peer).unwrap().pings.get(),
            // MIN_NUM_PINGS is defined differently based on cfg(debug_assertions), and tests in
            // garnix CI are run with release mode, so we support both cases.
            match MIN_NUM_PINGS {
                1 => Some(HpFixed::from(50)),
                3 => Some(HpFixed::from(33.33333333333333)),
                _ => panic!("MIN_NUM_PINGS is different than expected"),
            }
        );
    }

    #[test]
    fn test_lateny_min_max() {
        let mut rng = random::get_seedable_rng();
        let peers: Vec<NodeIndex> = (0..10).collect();
        let mut manager = MeasurementManager::new();

        for _ in 0..100 {
            let measurements =
                reputation::generate_reputation_measurements(&mut rng, PROB_MEASUREMENT_PRESENT);
            let index = rng.gen_range(0..peers.len());
            let peer = peers[index];
            if let Some(latency) = measurements.latency {
                manager.report_latency(peer, latency);
            }
        }
        let mut min_val = Duration::from_millis(u64::MAX);
        let mut max_val = Duration::from_millis(0);
        manager.peers.iter().for_each(|(_, measurements)| {
            if let Some(value) = measurements.latency.get() {
                min_val = min_val.min(value);
                max_val = max_val.max(value);
            }
        });
        assert_eq!(manager.summary_stats.min_latency().unwrap(), min_val);
        assert_eq!(manager.summary_stats.max_latency().unwrap(), max_val);
    }

    #[test]
    fn test_interactions_min_max() {
        let mut rng = random::get_seedable_rng();
        let peers: Vec<NodeIndex> = (0..10).collect();
        let mut manager = MeasurementManager::new();

        for _ in 0..100 {
            let index = rng.gen_range(0..peers.len());
            let peer = peers[index];
            if rng.gen_bool(0.5) {
                manager.report_sat(peer, Weight::Weak);
            } else {
                manager.report_unsat(peer, Weight::Weak);
            }
        }
        let mut min_val = i64::MAX;
        let mut max_val = i64::MIN;
        manager.peers.iter().for_each(|(_, measurements)| {
            if let Some(value) = measurements.interactions.get() {
                min_val = min_val.min(value);
                max_val = max_val.max(value);
            }
        });
        assert_eq!(manager.summary_stats.min_interactions().unwrap(), min_val);
        assert_eq!(manager.summary_stats.max_interactions().unwrap(), max_val);
    }

    #[test]
    fn test_bytes_received_min_max() {
        let mut rng = random::get_seedable_rng();
        let peers: Vec<NodeIndex> = (0..10).collect();
        let mut manager = MeasurementManager::new();

        for _ in 0..100 {
            let index = rng.gen_range(0..peers.len());
            let peer = peers[index];
            let bytes = rng.gen_range(100..10000);
            let duration = Duration::from_millis(rng.gen_range(100..10000));
            manager.report_bytes_received(peer, bytes, Some(duration));
        }
        let mut min_val_br = u128::MAX;
        let mut max_val_br = u128::MIN;

        let mut min_val_ob = u128::MAX;
        let mut max_val_ob = u128::MIN;
        manager.peers.iter().for_each(|(_, measurements)| {
            min_val_br = min_val_br.min(measurements.bytes_received.get());
            max_val_br = max_val_br.max(measurements.bytes_received.get());

            if let Some(value) = measurements.outbound_bandwidth.get() {
                min_val_ob = min_val_ob.min(value);
                max_val_ob = max_val_ob.max(value);
            }
        });
        assert_eq!(
            manager.summary_stats.min_bytes_received().unwrap(),
            min_val_br
        );
        assert_eq!(
            manager.summary_stats.max_bytes_received().unwrap(),
            max_val_br
        );
        assert_eq!(
            manager.summary_stats.min_outbound_bandwidth().unwrap(),
            min_val_ob
        );
        assert_eq!(
            manager.summary_stats.max_outbound_bandwidth().unwrap(),
            max_val_ob
        );
    }

    #[test]
    fn test_bytes_sent_min_max() {
        let mut rng = random::get_seedable_rng();
        let peers: Vec<NodeIndex> = (0..10).collect();
        let mut manager = MeasurementManager::new();

        for _ in 0..100 {
            let index = rng.gen_range(0..peers.len());
            let peer = peers[index];
            let bytes = rng.gen_range(100..10000);
            let duration = Duration::from_millis(rng.gen_range(100..10000));
            manager.report_bytes_sent(peer, bytes, Some(duration));
        }
        let mut min_val_bs = u128::MAX;
        let mut max_val_bs = u128::MIN;

        let mut min_val_ib = u128::MAX;
        let mut max_val_ib = u128::MIN;
        manager.peers.iter().for_each(|(_, measurements)| {
            min_val_bs = min_val_bs.min(measurements.bytes_sent.get());
            max_val_bs = max_val_bs.max(measurements.bytes_sent.get());

            if let Some(value) = measurements.inbound_bandwidth.get() {
                min_val_ib = min_val_ib.min(value);
                max_val_ib = max_val_ib.max(value);
            }
        });
        assert_eq!(manager.summary_stats.min_bytes_sent().unwrap(), min_val_bs);
        assert_eq!(manager.summary_stats.max_bytes_sent().unwrap(), max_val_bs);
        assert_eq!(
            manager.summary_stats.min_inbound_bandwidth().unwrap(),
            min_val_ib
        );
        assert_eq!(
            manager.summary_stats.max_inbound_bandwidth().unwrap(),
            max_val_ib
        );
    }

    #[test]
    fn test_hops_min_max() {
        let mut rng = random::get_seedable_rng();
        let peers: Vec<NodeIndex> = (0..10).collect();
        let mut manager = MeasurementManager::new();

        for _ in 0..100 {
            let index = rng.gen_range(0..peers.len());
            let peer = peers[index];
            let hops = rng.gen_range(3..10);
            manager.report_hops(peer, hops);
        }
        let mut min_val = u8::MAX;
        let mut max_val = u8::MIN;
        manager.peers.iter().for_each(|(_, measurements)| {
            if let Some(value) = measurements.hops.get() {
                min_val = min_val.min(value);
                max_val = max_val.max(value);
            }
        });
        assert_eq!(manager.summary_stats.min_hops().unwrap(), min_val);
        assert_eq!(manager.summary_stats.max_hops().unwrap(), max_val);
    }

    #[test]
    fn test_get_local_reputation_ref() {
        let mut manager = MeasurementManager::new();
        let peer1 = 0;
        manager.report_sat(peer1, Weight::Weak);
        let peer2 = 1;
        manager.report_sat(peer2, Weight::Strong);
        let reputation_map = manager.get_local_reputation_ref();
        assert!(reputation_map.contains(&peer2));
    }

    #[test]
    fn test_get_measurements_contains() {
        let mut manager = MeasurementManager::new();
        let peer1 = 1;
        manager.report_sat(peer1, Weight::Weak);
        let peer2 = 0;
        manager.report_sat(peer2, Weight::Weak);
        let peer_measurements = manager.get_measurements();
        assert!(peer_measurements.contains_key(&peer1));
        assert!(peer_measurements.contains_key(&peer2));
    }

    #[test]
    fn test_get_measurements_equals() {
        let mut manager = MeasurementManager::new();
        let peer = 0;
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
        let peer = 0;
        manager.report_sat(peer, Weight::Weak);
        let peer_measurements = manager.get_measurements();
        assert!(peer_measurements.contains_key(&peer));
        manager.clear_measurements();
        let peer_measurements = manager.get_measurements();
        assert!(!peer_measurements.contains_key(&peer));
    }

    #[test]
    fn test_normalized_measurements_in_range() {
        let mut summary_stats = SummaryStatistics::default();
        let rep_measurements = generate_weighted_measurements(20);
        rep_measurements.iter().for_each(|m| {
            summary_stats.add_latency(m.latency);
            summary_stats.add_interactions(m.interactions);
            summary_stats.add_inbound_bandwidth(m.inbound_bandwidth);
            summary_stats.add_outbound_bandwidth(m.outbound_bandwidth);
            if let Some(bytes_received) = m.bytes_received {
                summary_stats.add_bytes_received(bytes_received);
            }
            if let Some(bytes_sent) = m.bytes_sent {
                summary_stats.add_bytes_sent(bytes_sent);
            }
            summary_stats.add_hops(m.hops);
        });
        for rm in rep_measurements {
            let normalized_measurements = NormalizedMeasurements::new(rm, &summary_stats);
            if let Some(latency) = normalized_measurements.latency {
                assert!((0.0..=1.0).contains(&latency));
            }
            if let Some(interactions) = normalized_measurements.interactions {
                assert!((0.0..=1.0).contains(&interactions));
            }
            if let Some(inbound_bandwidth) = normalized_measurements.inbound_bandwidth {
                assert!((0.0..=1.0).contains(&inbound_bandwidth));
            }
            if let Some(outbound_bandwidth) = normalized_measurements.outbound_bandwidth {
                assert!((0.0..=1.0).contains(&outbound_bandwidth));
            }
            if let Some(bytes_received) = normalized_measurements.bytes_received {
                assert!((0.0..=1.0).contains(&bytes_received));
            }
            if let Some(bytes_sent) = normalized_measurements.bytes_sent {
                assert!((0.0..=1.0).contains(&bytes_sent));
            }
            if let Some(uptime) = normalized_measurements.uptime {
                assert!((0.0..=1.0).contains(&uptime));
            }
        }
    }
}
