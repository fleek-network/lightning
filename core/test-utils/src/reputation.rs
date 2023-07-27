use std::time::Duration;

use freek_interfaces::types::ReputationMeasurements;
use rand::{rngs::StdRng, Rng};

pub fn generate_reputation_measurements(
    rng: &mut StdRng,
    prob_measurement_present: f64,
) -> ReputationMeasurements {
    let latency = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(Duration::from_millis(rng.gen_range(100..=400)))
    };
    let interactions = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(rng.gen_range(-20..=100))
    };
    let inbound_bandwidth = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        // bytes per milliseconds: 50 Mbps to 250 Mbps
        Some(rng.gen_range(6250..31250))
    };
    let outbound_bandwidth = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        // bytes per milliseconds: 50 Mbps to 250 Mbps
        Some(rng.gen_range(6250..31250))
    };
    let bytes_received = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(rng.gen_range(100_000..1_000_000_000))
    };
    let bytes_sent = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(rng.gen_range(100_000..1_000_000_000))
    };
    ReputationMeasurements {
        latency,
        interactions,
        inbound_bandwidth,
        outbound_bandwidth,
        bytes_received,
        bytes_sent,
        hops: None,
    }
}
