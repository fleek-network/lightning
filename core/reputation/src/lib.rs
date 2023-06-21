use std::collections::HashMap;

use fleek_crypto::NodePublicKey;

pub mod statistics;
#[cfg(test)]
mod test_utils;
pub mod types;

use types::{
    CollectedMeasurements, MinMaxValues, NormalizedMeasurements, WeightedReputationMeasurements,
};

pub fn calculate_reputation_scores(
    weighted_measurements_map: HashMap<NodePublicKey, Vec<WeightedReputationMeasurements>>,
) -> HashMap<NodePublicKey, u8> {
    let mut normalized_measurements_map =
        calculate_normalized_measurements(weighted_measurements_map);

    let min_max_vals: MinMaxValues = (&normalized_measurements_map).into();

    normalized_measurements_map
        .iter_mut()
        .for_each(|(_, m)| m.min_max_normalize(min_max_vals.clone()));

    normalized_measurements_map
        .iter()
        .filter_map(|(node, m)| m.calculate_score().map(|score| (*node, score)))
        .collect()
}

fn calculate_normalized_measurements(
    weighted_measurements_map: HashMap<NodePublicKey, Vec<WeightedReputationMeasurements>>,
) -> HashMap<NodePublicKey, NormalizedMeasurements> {
    weighted_measurements_map
        .into_iter()
        .map(|(node, rm)| {
            let collected_measurements: CollectedMeasurements = rm.into();
            let normalized_measurements: NormalizedMeasurements = collected_measurements.into();
            (node, normalized_measurements)
        })
        .collect()
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{test_utils::*, types::*};

    const EPSILON: f64 = 1e-8;

    #[test]
    fn test_from_weighted_measurements_for_collected_measurements_counts() {
        let weighted_measurements = generate_weighted_measurements(10);
        let mut latency_count = 0;
        let mut interactions_count = 0;
        let mut inbound_bandwidth_count = 0;
        let mut outbound_bandwidth_count = 0;
        let mut bytes_received_count = 0;
        let mut bytes_sent_count = 0;
        for rm in &weighted_measurements {
            if rm.measurements.latency.is_some() {
                latency_count += 1;
            }
            if rm.measurements.interactions.is_some() {
                interactions_count += 1;
            }
            if rm.measurements.inbound_bandwidth.is_some() {
                inbound_bandwidth_count += 1;
            }
            if rm.measurements.outbound_bandwidth.is_some() {
                outbound_bandwidth_count += 1;
            }
            if rm.measurements.bytes_received.is_some() {
                bytes_received_count += 1;
            }
            if rm.measurements.bytes_sent.is_some() {
                bytes_sent_count += 1;
            }
        }
        let collected_measurements: CollectedMeasurements = weighted_measurements.into();
        assert_eq!(collected_measurements.latency.len(), latency_count);
        assert_eq!(
            collected_measurements.interactions.len(),
            interactions_count
        );
        assert_eq!(
            collected_measurements.inbound_bandwidth.len(),
            inbound_bandwidth_count
        );
        assert_eq!(
            collected_measurements.outbound_bandwidth.len(),
            outbound_bandwidth_count
        );
        assert_eq!(
            collected_measurements.bytes_received.len(),
            bytes_received_count
        );
        assert_eq!(collected_measurements.bytes_sent.len(), bytes_sent_count);
    }

    #[test]
    fn test_from_weighted_measurements_for_collected_measurements_weights_sum_to_1() {
        let weighted_measurements = generate_weighted_measurements(10);
        let collected_measurements: CollectedMeasurements = weighted_measurements.into();

        let mut weight_sum = 0.0;
        collected_measurements.latency.iter().for_each(|w| {
            assert!((0.0..=1.0).contains(&w.weight));
            weight_sum += w.weight;
        });
        assert!((weight_sum - 1.0).abs() < EPSILON);

        let mut weight_sum = 0.0;
        collected_measurements.latency.iter().for_each(|w| {
            assert!((0.0..=1.0).contains(&w.weight));
            weight_sum += w.weight;
        });
        assert!((weight_sum - 1.0).abs() < EPSILON);

        let mut weight_sum = 0.0;
        collected_measurements
            .inbound_bandwidth
            .iter()
            .for_each(|w| {
                assert!((0.0..=1.0).contains(&w.weight));
                weight_sum += w.weight;
            });
        assert!((weight_sum - 1.0).abs() < EPSILON);

        let mut weight_sum = 0.0;
        collected_measurements
            .outbound_bandwidth
            .iter()
            .for_each(|w| {
                assert!((0.0..=1.0).contains(&w.weight));
                weight_sum += w.weight;
            });
        assert!((weight_sum - 1.0).abs() < EPSILON);

        let mut weight_sum = 0.0;
        collected_measurements.bytes_received.iter().for_each(|w| {
            assert!((0.0..=1.0).contains(&w.weight));
            weight_sum += w.weight;
        });
        assert!((weight_sum - 1.0).abs() < EPSILON);

        let mut weight_sum = 0.0;
        collected_measurements.bytes_sent.iter().for_each(|w| {
            assert!((0.0..=1.0).contains(&w.weight));
            weight_sum += w.weight;
        });
        assert!((weight_sum - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_weighted_measurements_for_collected_measurements_values() {
        let weighted_measurements = generate_weighted_measurements(1);
        let collected_measurements: CollectedMeasurements = weighted_measurements.clone().into();
        if let Some(latency) = weighted_measurements[0].measurements.latency {
            assert_eq!(
                latency.as_millis() as f64,
                collected_measurements.latency[0].value
            );
        }
        if let Some(interactions) = weighted_measurements[0].measurements.interactions {
            assert_eq!(
                interactions as f64,
                collected_measurements.interactions[0].value
            );
        }
        if let Some(inbound_bandwidth) = weighted_measurements[0].measurements.inbound_bandwidth {
            assert_eq!(
                inbound_bandwidth as f64,
                collected_measurements.inbound_bandwidth[0].value
            );
        }
        if let Some(outbound_bandwidth) = weighted_measurements[0].measurements.outbound_bandwidth {
            assert_eq!(
                outbound_bandwidth as f64,
                collected_measurements.outbound_bandwidth[0].value
            );
        }
        if let Some(bytes_received) = weighted_measurements[0].measurements.bytes_received {
            assert_eq!(
                bytes_received as f64,
                collected_measurements.bytes_received[0].value
            );
        }
        if let Some(bytes_sent) = weighted_measurements[0].measurements.bytes_sent {
            assert_eq!(
                bytes_sent as f64,
                collected_measurements.bytes_sent[0].value
            );
        }
    }

    #[test]
    fn test_from_collected_measurements_for_normalized_measurements() {
        let weighted_measurements = generate_weighted_measurements(10);
        let mut collected_measurements: CollectedMeasurements = weighted_measurements.into();
        collected_measurements.outbound_bandwidth = Vec::new();

        let normalized_measurements: NormalizedMeasurements = collected_measurements.clone().into();
        if collected_measurements.latency.is_empty() {
            assert!(normalized_measurements.latency.is_none());
        } else {
            assert!(normalized_measurements.latency.is_some());
        }
        if collected_measurements.interactions.is_empty() {
            assert!(normalized_measurements.interactions.is_none());
        } else {
            assert!(normalized_measurements.interactions.is_some());
        }
        if collected_measurements.inbound_bandwidth.is_empty() {
            assert!(normalized_measurements.inbound_bandwidth.is_none());
        } else {
            assert!(normalized_measurements.inbound_bandwidth.is_some());
        }
        if collected_measurements.outbound_bandwidth.is_empty() {
            assert!(normalized_measurements.outbound_bandwidth.is_none());
        } else {
            assert!(normalized_measurements.outbound_bandwidth.is_some());
        }
        if collected_measurements.bytes_received.is_empty() {
            assert!(normalized_measurements.bytes_received.is_none());
        } else {
            assert!(normalized_measurements.bytes_received.is_some());
        }
        if collected_measurements.bytes_sent.is_empty() {
            assert!(normalized_measurements.bytes_sent.is_none());
        } else {
            assert!(normalized_measurements.bytes_sent.is_some());
        }
    }

    #[test]
    fn test_normalized_measurements_min_max_normalize() {
        let weighted_measurements_map = generate_weighted_measurements_map(10);
        let mut normalized_measurements_map =
            calculate_normalized_measurements(weighted_measurements_map);

        let min_max_vals: MinMaxValues = (&normalized_measurements_map).into();

        normalized_measurements_map.iter_mut().for_each(|(_, m)| {
            m.min_max_normalize(min_max_vals.clone());
            if let Some(latency) = &m.latency {
                assert!((0.0..=1.0).contains(latency));
            }
            if let Some(interactions) = &m.interactions {
                assert!((0.0..=1.0).contains(interactions));
            }
            if let Some(inbound_bandwidth) = &m.inbound_bandwidth {
                assert!((0.0..=1.0).contains(inbound_bandwidth));
            }
            if let Some(outbound_bandwidth) = &m.outbound_bandwidth {
                assert!((0.0..=1.0).contains(outbound_bandwidth));
            }
            if let Some(bytes_received) = &m.bytes_received {
                assert!((0.0..=1.0).contains(bytes_received));
            }
            if let Some(bytes_sent) = &m.bytes_sent {
                assert!((0.0..=1.0).contains(bytes_sent));
            }
        });
    }

    #[test]
    fn test_calculate_score() {
        let normalized_measurements_map = generate_normalized_measurements_map(10);
        normalized_measurements_map.into_iter().for_each(|(_, m)| {
            if let Some(score) = &m.calculate_score() {
                assert!((0u8..=100u8).contains(score));
            }
        })
    }
}
