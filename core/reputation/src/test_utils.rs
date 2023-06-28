use std::collections::HashMap;

use draco_test_utils::{random, reputation};
use fleek_crypto::NodePublicKey;
use rand::{rngs::StdRng, Rng};

use crate::{NormalizedMeasurements, WeightedReputationMeasurements};

const PROB_MEASUREMENT_PRESENT: f64 = 0.1;

pub(crate) fn generate_weighted_measurements_map(
    map_size: usize,
    rng: Option<StdRng>,
) -> HashMap<NodePublicKey, Vec<WeightedReputationMeasurements>> {
    let mut rng = if let Some(rng) = rng {
        rng
    } else {
        random::get_seedable_rng()
    };
    let mut map = HashMap::with_capacity(map_size);
    for _ in 0..map_size {
        let mut array = [0; 96];
        (0..96).for_each(|i| array[i] = rng.gen_range(0..=255));
        let node = NodePublicKey(array);
        let num_measurements = rng.gen_range(1..20);
        map.insert(
            node,
            generate_weighted_measurements(num_measurements, Some(rng.clone())),
        );
    }
    map
}

pub(crate) fn generate_weighted_measurements(
    num_measurements: usize,
    rng: Option<StdRng>,
) -> Vec<WeightedReputationMeasurements> {
    let mut rng = if let Some(rng) = rng {
        rng
    } else {
        random::get_seedable_rng()
    };

    let mut reported_measurements = Vec::with_capacity(num_measurements);
    for _ in 0..num_measurements {
        let measurements =
            reputation::generate_reputation_measurements(&mut rng, PROB_MEASUREMENT_PRESENT);
        let weight = rng.gen_range(0..=100);

        let reported_measurement = WeightedReputationMeasurements {
            measurements,
            weight,
        };
        reported_measurements.push(reported_measurement);
    }
    reported_measurements
}

pub(crate) fn generate_normalized_measurements_map(
    map_size: usize,
) -> HashMap<NodePublicKey, NormalizedMeasurements> {
    let mut map = HashMap::with_capacity(map_size);
    let mut rng = random::get_seedable_rng();
    for _ in 0..map_size {
        let latency = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0).into())
        };
        let interactions = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0).into())
        };
        let inbound_bandwidth = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0).into())
        };
        let outbound_bandwidth = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0).into())
        };
        let bytes_received = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0).into())
        };
        let bytes_sent = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0).into())
        };
        let normalized_measurements = NormalizedMeasurements {
            latency,
            interactions,
            inbound_bandwidth,
            outbound_bandwidth,
            bytes_received,
            bytes_sent,
        };
        let mut array = [0; 96];
        (0..96).for_each(|i| array[i] = rng.gen_range(0..=255));
        let node = NodePublicKey(array);
        map.insert(node, normalized_measurements);
    }
    map
}
