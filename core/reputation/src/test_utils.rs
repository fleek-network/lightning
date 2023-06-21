use std::{collections::HashMap, time::Duration};

use draco_interfaces::types::ReputationMeasurements;
use fleek_crypto::NodePublicKey;
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::{NormalizedMeasurements, WeightedReputationMeasurements};

const PROB_MEASUREMENT_PRESENT: f64 = 0.1;

pub(crate) fn get_seedable_rng() -> StdRng {
    let seed: [u8; 32] = (0..32).collect::<Vec<u8>>().try_into().unwrap();
    SeedableRng::from_seed(seed)
}

pub(crate) fn generate_reputation_measurements(rng: Option<StdRng>) -> ReputationMeasurements {
    let mut rng = if let Some(rng) = rng {
        rng
    } else {
        get_seedable_rng()
    };
    let latency = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
        None
    } else {
        Some(Duration::from_millis(rng.gen_range(100..=400)))
    };
    let interactions = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
        None
    } else {
        Some(rng.gen_range(-20..=100))
    };
    let inbound_bandwidth = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
        None
    } else {
        // bytes per milliseconds: 50 Mbps to 250 Mbps
        Some(rng.gen_range(6250..31250))
    };
    let outbound_bandwidth = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
        None
    } else {
        // bytes per milliseconds: 50 Mbps to 250 Mbps
        Some(rng.gen_range(6250..31250))
    };
    let bytes_received = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
        None
    } else {
        Some(rng.gen_range(100_000..1_000_000_000))
    };
    let bytes_sent = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
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

pub(crate) fn generate_weighted_measurements_map(
    map_size: usize,
    rng: Option<StdRng>,
) -> HashMap<NodePublicKey, Vec<WeightedReputationMeasurements>> {
    let mut rng = if let Some(rng) = rng {
        rng
    } else {
        get_seedable_rng()
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
        get_seedable_rng()
    };

    let mut reported_measurements = Vec::with_capacity(num_measurements);
    for _ in 0..num_measurements {
        let measurements = generate_reputation_measurements(Some(rng.clone()));
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
    let mut rng = get_seedable_rng();
    for _ in 0..map_size {
        let latency = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0))
        };
        let interactions = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0))
        };
        let inbound_bandwidth = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0))
        };
        let outbound_bandwidth = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0))
        };
        let bytes_received = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0))
        };
        let bytes_sent = if rng.gen_bool(PROB_MEASUREMENT_PRESENT) {
            None
        } else {
            Some(rng.gen_range(0.0..=1.0))
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
