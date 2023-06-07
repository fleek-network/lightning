use std::{collections::HashMap, time::Duration};

use draco_interfaces::Weight;
use fleek_crypto::NodePublicKey;

/// Manages the measurements for all the peers.
pub struct MeasurementManager {
    peers: HashMap<NodePublicKey, Measurements>,
}

impl MeasurementManager {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn get_reputation_of(&self, _peer: &NodePublicKey) -> Option<u128> {
        todo!()
    }

    pub fn report_sat(&mut self, peer: NodePublicKey, weight: Weight) {
        self.peers
            .entry(peer)
            .or_default()
            .register_interaction(true, weight);
    }

    pub fn report_unsat(&mut self, peer: NodePublicKey, weight: Weight) {
        self.peers
            .entry(peer)
            .or_default()
            .register_interaction(false, weight);
    }

    pub fn report_latency(&mut self, peer: NodePublicKey, latency: Duration) {
        self.peers
            .entry(peer)
            .or_default()
            .register_latency(latency);
    }

    pub fn report_bytes_received(
        &mut self,
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        let measurements = self.peers.entry(peer).or_default();
        measurements.register_bytes_received(bytes);
        if let Some(duration) = duration {
            measurements.register_outbound_bandwidth(bytes, duration);
        }
    }

    pub fn report_bytes_sent(
        &mut self,
        peer: NodePublicKey,
        bytes: u64,
        duration: Option<Duration>,
    ) {
        let measurements = self.peers.entry(peer).or_default();
        measurements.register_bytes_sent(bytes);
        if let Some(duration) = duration {
            measurements.register_inbound_bandwidth(bytes, duration);
        }
    }

    pub fn report_hops(&mut self, peer: NodePublicKey, hops: u8) {
        self.peers.entry(peer).or_default().register_hops(hops);
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
        self.bytes_sent.register_bytes_transferred(bytes);
    }

    fn register_bytes_sent(&mut self, bytes: u64) {
        self.bytes_received.register_bytes_transferred(bytes);
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
    sum: u32,
    count: u32,
}

impl Interactions {
    fn new() -> Self {
        Self { sum: 0, count: 0 }
    }

    fn register_interaction(&mut self, sat: bool, weight: Weight) {
        if sat {
            self.sum += Interactions::get_weight(weight);
        } else {
            self.sum -= Interactions::get_weight(weight);
        }
    }

    #[allow(dead_code)]
    fn get(&self) -> Option<f32> {
        if self.count > 0 {
            Some(self.sum as f32 / self.count as f32)
        } else {
            None
        }
    }

    fn get_weight(weight: Weight) -> u32 {
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
