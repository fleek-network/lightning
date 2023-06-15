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
                    f64::min(min_max_vals.min_values.latency, interactions);
                min_max_vals.max_values.interactions =
                    f64::max(min_max_vals.max_values.latency, interactions);
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
                min_max_vals.min_values.bytes_received =
                    f64::min(min_max_vals.min_values.bytes_sent, bytes_sent);
                min_max_vals.max_values.bytes_received =
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

    fn calculate_score(&self) -> f64 {
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

        score / count as f64
    }
}

pub fn calculate_reputation_scores(
    reported_measurements_map: HashMap<NodePublicKey, Vec<ReportedReputationMeasurements>>,
) -> HashMap<NodePublicKey, u8> {
    let mut normalized_measurements_map: HashMap<NodePublicKey, NormalizedMeasurements> =
        reported_measurements_map
            .into_iter()
            .map(|(node, rm)| {
                let collected_measurements: CollectedMeasurements = rm.into();
                let normalized_measurements: NormalizedMeasurements = collected_measurements.into();
                (node, normalized_measurements)
            })
            .collect();
    let min_max_vals: MinMaxValues = (&normalized_measurements_map).into();

    normalized_measurements_map
        .iter_mut()
        .for_each(|(_, m)| m.min_max_normalize(min_max_vals.clone()));

    normalized_measurements_map
        .iter()
        .map(|(node, m)| (*node, (m.calculate_score() * 100.0) as u8))
        .collect()
}
