use std::collections::HashMap;

use draco_interfaces::types::ReportedReputationMeasurements;
use fleek_crypto::NodePublicKey;

pub mod statistics;

#[derive(Debug, Clone, Default)]
struct CollectedMeasurements {
    latency: Vec<f64>,
    interactions: Vec<f64>,
    inbound_bandwidth: Vec<f64>,
    outbound_bandwidth: Vec<f64>,
    bytes_received: Vec<f64>,
    bytes_sent: Vec<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct NormalizedMeasurements {
    pub latency: Option<f64>,
    pub interactions: Option<f64>,
    pub inbound_bandwidth: Option<f64>,
    pub outbound_bandwidth: Option<f64>,
    pub bytes_received: Option<f64>,
    pub bytes_sent: Option<f64>,
}

#[derive(Debug, Clone, Default)]
struct Values {
    latency: f64,
    interactions: f64,
    inbound_bandwidth: f64,
    outbound_bandwidth: f64,
    bytes_received: f64,
    bytes_sent: f64,
}

#[derive(Debug, Clone, Default)]
struct MinMaxValues {
    min_values: Values,
    max_values: Values,
}

impl From<Vec<ReportedReputationMeasurements>> for CollectedMeasurements {
    fn from(reported_measurements: Vec<ReportedReputationMeasurements>) -> Self {
        let mut measurements = Self::default();
        reported_measurements.into_iter().for_each(|rm| {
            if let Some(latency) = rm.measurements.latency {
                measurements.latency.push(latency.as_millis() as f64);
            }
            if let Some(interactions) = rm.measurements.interactions {
                measurements.interactions.push(interactions as f64);
            }
            if let Some(inbound_bandwidth) = rm.measurements.inbound_bandwidth {
                measurements
                    .inbound_bandwidth
                    .push(inbound_bandwidth as f64);
            }
            if let Some(outbound_bandwidth) = rm.measurements.outbound_bandwidth {
                measurements
                    .outbound_bandwidth
                    .push(outbound_bandwidth as f64);
            }
            if let Some(bytes_received) = rm.measurements.bytes_received {
                measurements.bytes_received.push(bytes_received as f64);
            }
            if let Some(bytes_sent) = rm.measurements.bytes_sent {
                measurements.bytes_sent.push(bytes_sent as f64);
            }
        });
        measurements
    }
}

impl From<&HashMap<NodePublicKey, NormalizedMeasurements>> for MinMaxValues {
    fn from(normalized_measurements: &HashMap<NodePublicKey, NormalizedMeasurements>) -> Self {
        let min_values = Values {
            latency: f64::MAX,
            interactions: f64::MAX,
            inbound_bandwidth: f64::MAX,
            outbound_bandwidth: f64::MAX,
            bytes_received: f64::MAX,
            bytes_sent: f64::MAX,
        };
        let max_values = Values {
            latency: f64::MIN,
            interactions: f64::MIN,
            inbound_bandwidth: f64::MIN,
            outbound_bandwidth: f64::MIN,
            bytes_received: f64::MIN,
            bytes_sent: f64::MIN,
        };
        let mut min_max_vals = MinMaxValues {
            min_values,
            max_values,
        };

        normalized_measurements.iter().for_each(|(_, m)| {
            if let Some(latency) = m.latency {
                min_max_vals.min_values.latency =
                    f64::min(min_max_vals.min_values.latency, latency);
                min_max_vals.max_values.latency =
                    f64::max(min_max_vals.max_values.latency, latency);
            }
            if let Some(interactions) = m.interactions {
                min_max_vals.min_values.interactions =
                    f64::min(min_max_vals.min_values.interactions, interactions);
                min_max_vals.max_values.interactions =
                    f64::max(min_max_vals.max_values.interactions, interactions);
            }
            if let Some(inbound_bandwidth) = m.inbound_bandwidth {
                min_max_vals.min_values.inbound_bandwidth =
                    f64::min(min_max_vals.min_values.inbound_bandwidth, inbound_bandwidth);
                min_max_vals.max_values.inbound_bandwidth =
                    f64::max(min_max_vals.max_values.inbound_bandwidth, inbound_bandwidth);
            }
            if let Some(outbound_bandwidth) = m.outbound_bandwidth {
                min_max_vals.min_values.outbound_bandwidth = f64::min(
                    min_max_vals.min_values.outbound_bandwidth,
                    outbound_bandwidth,
                );
                min_max_vals.max_values.outbound_bandwidth = f64::max(
                    min_max_vals.max_values.outbound_bandwidth,
                    outbound_bandwidth,
                );
            }
            if let Some(bytes_received) = m.bytes_received {
                min_max_vals.min_values.bytes_received =
                    f64::min(min_max_vals.min_values.bytes_received, bytes_received);
                min_max_vals.max_values.bytes_received =
                    f64::max(min_max_vals.max_values.bytes_received, bytes_received);
            }
            if let Some(bytes_sent) = m.bytes_sent {
                min_max_vals.min_values.bytes_sent =
                    f64::min(min_max_vals.min_values.bytes_sent, bytes_sent);
                min_max_vals.max_values.bytes_sent =
                    f64::max(min_max_vals.max_values.bytes_sent, bytes_sent);
            }
        });
        min_max_vals
    }
}

impl From<CollectedMeasurements> for NormalizedMeasurements {
    fn from(collected_measurements: CollectedMeasurements) -> Self {
        let latency = if collected_measurements.latency.is_empty() {
            None
        } else {
            Some(statistics::calculate_normalized_mean(
                collected_measurements.latency,
            ))
        };
        let interactions = if collected_measurements.interactions.is_empty() {
            None
        } else {
            Some(statistics::calculate_normalized_mean(
                collected_measurements.interactions,
            ))
        };
        let inbound_bandwidth = if collected_measurements.inbound_bandwidth.is_empty() {
            None
        } else {
            Some(statistics::calculate_normalized_mean(
                collected_measurements.inbound_bandwidth,
            ))
        };
        let outbound_bandwidth = if collected_measurements.outbound_bandwidth.is_empty() {
            None
        } else {
            Some(statistics::calculate_normalized_mean(
                collected_measurements.outbound_bandwidth,
            ))
        };
        let bytes_received = if collected_measurements.bytes_received.is_empty() {
            None
        } else {
            Some(statistics::calculate_normalized_mean(
                collected_measurements.bytes_received,
            ))
        };
        let bytes_sent = if collected_measurements.bytes_sent.is_empty() {
            None
        } else {
            Some(statistics::calculate_normalized_mean(
                collected_measurements.bytes_sent,
            ))
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

impl NormalizedMeasurements {
    fn min_max_normalize(&mut self, min_max_vals: MinMaxValues) {
        if let Some(latency) = self.latency {
            self.latency = statistics::try_min_max_normalize(
                latency,
                min_max_vals.min_values.latency,
                min_max_vals.max_values.latency,
            );
        }
        if let Some(interactions) = self.interactions {
            self.interactions = statistics::try_min_max_normalize(
                interactions,
                min_max_vals.min_values.interactions,
                min_max_vals.max_values.interactions,
            );
        }
        if let Some(inbound_bandwidth) = self.inbound_bandwidth {
            self.inbound_bandwidth = statistics::try_min_max_normalize(
                inbound_bandwidth,
                min_max_vals.min_values.inbound_bandwidth,
                min_max_vals.max_values.inbound_bandwidth,
            );
        }
        if let Some(outbound_bandwidth) = self.outbound_bandwidth {
            self.outbound_bandwidth = statistics::try_min_max_normalize(
                outbound_bandwidth,
                min_max_vals.min_values.outbound_bandwidth,
                min_max_vals.max_values.outbound_bandwidth,
            );
        }
        if let Some(bytes_received) = self.bytes_received {
            self.bytes_received = statistics::try_min_max_normalize(
                bytes_received,
                min_max_vals.min_values.bytes_received,
                min_max_vals.max_values.bytes_received,
            );
        }
        if let Some(bytes_sent) = self.bytes_sent {
            self.bytes_sent = statistics::try_min_max_normalize(
                bytes_sent,
                min_max_vals.min_values.bytes_sent,
                min_max_vals.max_values.bytes_sent,
            );
        }
    }

    fn calculate_score(&self) -> Option<u8> {
        let mut score = 0.0;
        let mut count = 0;
        if let Some(latency) = self.latency {
            score += 1.0 - latency;
            count += 1;
        }
        if let Some(interactions) = self.interactions {
            score += interactions;
            count += 1;
        }
        if let Some(inbound_bandwidth) = self.inbound_bandwidth {
            score += inbound_bandwidth;
            count += 1;
        }
        if let Some(outbound_bandwidth) = self.outbound_bandwidth {
            score += outbound_bandwidth;
            count += 1;
        }
        if let Some(bytes_received) = self.bytes_received {
            score += bytes_received;
            count += 1;
        }
        if let Some(bytes_sent) = self.bytes_sent {
            score += bytes_sent;
            count += 1;
        }

        if count == 0 {
            return None;
        }
        // This value will be in the range [0, 1]
        score /= count as f64;
        Some((score * 100.0) as u8)
    }
}

fn calculate_normalized_measurements(
    reported_measurements_map: HashMap<NodePublicKey, Vec<ReportedReputationMeasurements>>,
) -> HashMap<NodePublicKey, NormalizedMeasurements> {
    reported_measurements_map
        .into_iter()
        .map(|(node, rm)| {
            let collected_measurements: CollectedMeasurements = rm.into();
            let normalized_measurements: NormalizedMeasurements = collected_measurements.into();
            (node, normalized_measurements)
        })
        .collect()
}

pub fn calculate_reputation_scores(
    reported_measurements_map: HashMap<NodePublicKey, Vec<ReportedReputationMeasurements>>,
) -> HashMap<NodePublicKey, u8> {
    let mut normalized_measurements_map =
        calculate_normalized_measurements(reported_measurements_map);

    let min_max_vals: MinMaxValues = (&normalized_measurements_map).into();

    normalized_measurements_map
        .iter_mut()
        .for_each(|(_, m)| m.min_max_normalize(min_max_vals.clone()));

    normalized_measurements_map
        .iter()
        .filter_map(|(node, m)| m.calculate_score().map(|score| (*node, score)))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use draco_interfaces::types::ReputationMeasurements;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    use super::*;

    const PROB_MEASUREMENT_PRESENT: f64 = 0.1;

    fn get_seedable_rng() -> StdRng {
        let seed: [u8; 32] = (0..32).collect::<Vec<u8>>().try_into().unwrap();
        SeedableRng::from_seed(seed)
    }

    fn generate_reported_measurements_map(
        map_size: usize,
    ) -> HashMap<NodePublicKey, Vec<ReportedReputationMeasurements>> {
        let mut map = HashMap::with_capacity(map_size);
        let mut rng = get_seedable_rng();
        for _ in 0..map_size {
            let mut array = [0; 96];
            (0..96).for_each(|i| array[i] = rng.gen_range(0..=255));
            let node = NodePublicKey(array);
            let num_measurements = rng.gen_range(1..20);
            map.insert(node, generate_reported_measurements(num_measurements));
        }
        map
    }

    fn generate_reported_measurements(
        num_measurements: usize,
    ) -> Vec<ReportedReputationMeasurements> {
        let mut rng = get_seedable_rng();

        let mut reported_measurements = Vec::with_capacity(num_measurements);
        for _ in 0..num_measurements {
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

            let measurements = ReputationMeasurements {
                latency,
                interactions,
                inbound_bandwidth,
                outbound_bandwidth,
                bytes_received,
                bytes_sent,
                hops: None,
            };
            let mut array = [0; 96];
            (0..96).for_each(|i| array[i] = rng.gen_range(0..=255));
            let reporting_node = NodePublicKey(array);

            let reported_measurement = ReportedReputationMeasurements {
                reporting_node,
                measurements,
            };
            reported_measurements.push(reported_measurement);
        }

        reported_measurements
    }

    fn generate_normalized_measurements_map(
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

    #[test]
    fn test_from_reported_measurements_for_collected_measurements_counts() {
        let reported_measurements = generate_reported_measurements(10);
        let mut latency_count = 0;
        let mut interactions_count = 0;
        let mut inbound_bandwidth_count = 0;
        let mut outbound_bandwidth_count = 0;
        let mut bytes_received_count = 0;
        let mut bytes_sent_count = 0;
        for rm in &reported_measurements {
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
        let collected_measurements: CollectedMeasurements = reported_measurements.into();
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
    fn test_from_reported_measurements_for_collected_measurements_values() {
        let reported_measurements = generate_reported_measurements(1);
        let collected_measurements: CollectedMeasurements = reported_measurements.clone().into();
        if let Some(latency) = reported_measurements[0].measurements.latency {
            assert_eq!(
                latency.as_millis() as f64,
                collected_measurements.latency[0]
            );
        }
        if let Some(interactions) = reported_measurements[0].measurements.interactions {
            assert_eq!(interactions as f64, collected_measurements.interactions[0]);
        }
        if let Some(inbound_bandwidth) = reported_measurements[0].measurements.inbound_bandwidth {
            assert_eq!(
                inbound_bandwidth as f64,
                collected_measurements.inbound_bandwidth[0]
            );
        }
        if let Some(outbound_bandwidth) = reported_measurements[0].measurements.outbound_bandwidth {
            assert_eq!(
                outbound_bandwidth as f64,
                collected_measurements.outbound_bandwidth[0]
            );
        }
        if let Some(bytes_received) = reported_measurements[0].measurements.bytes_received {
            assert_eq!(
                bytes_received as f64,
                collected_measurements.bytes_received[0]
            );
        }
        if let Some(bytes_sent) = reported_measurements[0].measurements.bytes_sent {
            assert_eq!(bytes_sent as f64, collected_measurements.bytes_sent[0]);
        }
    }

    #[test]
    fn test_from_collected_measurements_for_normalized_measurements() {
        let reported_measurements = generate_reported_measurements(10);
        let mut collected_measurements: CollectedMeasurements = reported_measurements.into();
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
        let reported_measurements_map = generate_reported_measurements_map(10);
        let mut normalized_measurements_map =
            calculate_normalized_measurements(reported_measurements_map);

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
