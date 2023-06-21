use std::{
    collections::HashMap,
    ops::{Add, Div, Mul, Sub},
};

use draco_interfaces::types::ReputationMeasurements;
use fleek_crypto::NodePublicKey;

use crate::statistics;

pub trait WeightedValue {
    fn get_weighted_value(&self) -> f64;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WeightedFloat {
    pub(crate) value: f64,
    pub(crate) weight: f64,
}

impl WeightedValue for WeightedFloat {
    fn get_weighted_value(&self) -> f64 {
        self.value * self.weight
    }
}

impl Default for WeightedFloat {
    fn default() -> Self {
        WeightedFloat {
            value: 0.0,
            weight: 1.0,
        }
    }
}

impl Add<WeightedFloat> for WeightedFloat {
    type Output = WeightedFloat;

    fn add(self, rhs: WeightedFloat) -> Self::Output {
        WeightedFloat {
            value: self.value + rhs.value,
            weight: self.weight,
        }
    }
}

impl Div for WeightedFloat {
    type Output = WeightedFloat;

    fn div(self, rhs: Self) -> Self::Output {
        WeightedFloat {
            value: self.value / rhs.value,
            weight: self.weight,
        }
    }
}

impl Mul for WeightedFloat {
    type Output = WeightedFloat;

    fn mul(self, rhs: Self) -> Self::Output {
        WeightedFloat {
            value: self.value * rhs.value,
            weight: self.weight,
        }
    }
}

impl Sub for WeightedFloat {
    type Output = WeightedFloat;

    fn sub(self, rhs: Self) -> Self::Output {
        WeightedFloat {
            value: self.value - rhs.value,
            weight: self.weight,
        }
    }
}

impl From<f64> for WeightedFloat {
    fn from(value: f64) -> Self {
        WeightedFloat { value, weight: 1.0 }
    }
}

impl PartialEq for WeightedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl PartialOrd for WeightedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WeightedReputationMeasurements {
    pub measurements: ReputationMeasurements,
    pub weight: u8,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CollectedMeasurements {
    pub latency: Vec<WeightedFloat>,
    pub interactions: Vec<WeightedFloat>,
    pub inbound_bandwidth: Vec<WeightedFloat>,
    pub outbound_bandwidth: Vec<WeightedFloat>,
    pub bytes_received: Vec<WeightedFloat>,
    pub bytes_sent: Vec<WeightedFloat>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NormalizedMeasurements {
    pub latency: Option<f64>,
    pub interactions: Option<f64>,
    pub inbound_bandwidth: Option<f64>,
    pub outbound_bandwidth: Option<f64>,
    pub bytes_received: Option<f64>,
    pub bytes_sent: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Values {
    latency: f64,
    interactions: f64,
    inbound_bandwidth: f64,
    outbound_bandwidth: f64,
    bytes_received: f64,
    bytes_sent: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MinMaxValues {
    min_values: Values,
    max_values: Values,
}

impl From<Vec<WeightedReputationMeasurements>> for CollectedMeasurements {
    fn from(weighted_measurements: Vec<WeightedReputationMeasurements>) -> Self {
        let mut weight_sum_latency = 0.0;
        let mut weight_sum_interactions = 0.0;
        let mut weight_sum_inbound_bandwidth = 0.0;
        let mut weight_sum_outbound_bandwidth = 0.0;
        let mut weight_sum_bytes_received = 0.0;
        let mut weight_sum_bytes_sent = 0.0;

        let mut count_latency = 0;
        let mut count_interactions = 0;
        let mut count_inbound_bandwidth = 0;
        let mut count_outbound_bandwidth = 0;
        let mut count_bytes_received = 0;
        let mut count_bytes_sent = 0;
        weighted_measurements.iter().for_each(|m| {
            if m.measurements.latency.is_some() {
                weight_sum_latency += m.weight as f64;
                count_latency += 1;
            }
            if m.measurements.interactions.is_some() {
                weight_sum_interactions += m.weight as f64;
                count_interactions += 1;
            }
            if m.measurements.inbound_bandwidth.is_some() {
                weight_sum_inbound_bandwidth += m.weight as f64;
                count_inbound_bandwidth += 1;
            }
            if m.measurements.outbound_bandwidth.is_some() {
                weight_sum_outbound_bandwidth += m.weight as f64;
                count_outbound_bandwidth += 1;
            }
            if m.measurements.bytes_received.is_some() {
                weight_sum_bytes_received += m.weight as f64;
                count_bytes_received += 1;
            }
            if m.measurements.bytes_sent.is_some() {
                weight_sum_bytes_sent += m.weight as f64;
                count_bytes_sent += 1;
            }
        });
        let mut measurements = Self::default();
        weighted_measurements.into_iter().for_each(|m| {
            if let Some(latency) = m.measurements.latency {
                let weight = if weight_sum_latency == 0.0 {
                    1.0 / count_latency as f64
                } else {
                    m.weight as f64 / weight_sum_latency
                };
                measurements.latency.push(WeightedFloat {
                    value: latency.as_millis() as f64,
                    weight,
                });
            }
            if let Some(interactions) = m.measurements.interactions {
                let weight = if weight_sum_interactions == 0.0 {
                    1.0 / count_interactions as f64
                } else {
                    m.weight as f64 / weight_sum_interactions
                };
                measurements.interactions.push(WeightedFloat {
                    value: interactions as f64,
                    weight,
                });
            }
            if let Some(inbound_bandwidth) = m.measurements.inbound_bandwidth {
                let weight = if weight_sum_inbound_bandwidth == 0.0 {
                    1.0 / count_inbound_bandwidth as f64
                } else {
                    m.weight as f64 / weight_sum_inbound_bandwidth
                };
                measurements.inbound_bandwidth.push(WeightedFloat {
                    value: inbound_bandwidth as f64,
                    weight,
                });
            }
            if let Some(outbound_bandwidth) = m.measurements.outbound_bandwidth {
                let weight = if weight_sum_outbound_bandwidth == 0.0 {
                    1.0 / count_outbound_bandwidth as f64
                } else {
                    m.weight as f64 / weight_sum_outbound_bandwidth
                };
                measurements.outbound_bandwidth.push(WeightedFloat {
                    value: outbound_bandwidth as f64,
                    weight,
                });
            }
            if let Some(bytes_received) = m.measurements.bytes_received {
                let weight = if weight_sum_bytes_received == 0.0 {
                    1.0 / count_bytes_received as f64
                } else {
                    m.weight as f64 / weight_sum_bytes_received
                };
                measurements.bytes_received.push(WeightedFloat {
                    value: bytes_received as f64,
                    weight,
                });
            }
            if let Some(bytes_sent) = m.measurements.bytes_sent {
                let weight = if weight_sum_bytes_sent == 0.0 {
                    1.0 / count_bytes_sent as f64
                } else {
                    m.weight as f64 / weight_sum_bytes_sent
                };
                measurements.bytes_sent.push(WeightedFloat {
                    value: bytes_sent as f64,
                    weight,
                });
            }
        });
        // For latency measurements, the lower the value, the better. Therefore, if we simply weight
        // the reported measurements by the reputation of the reporting node, we would put
        // more weight on latency measurements that were reported by nodes with lower
        // reputation scores.
        // To prevent this, we invert the weights that were derived from the reputation score for
        // latency measurements.
        if weight_sum_latency != 0.0 {
            let inverse_weight_sum = measurements
                .latency
                .iter()
                .fold(0.0, |acc, w| acc + (1.0 - w.weight));
            measurements.latency.iter_mut().for_each(|w| {
                w.weight = (1.0 - w.weight) / inverse_weight_sum;
            });
        }
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
                min_max_vals.min_values.latency = min_max_vals.min_values.latency.min(latency);
                min_max_vals.max_values.latency = min_max_vals.max_values.latency.max(latency);
            }
            if let Some(interactions) = m.interactions {
                min_max_vals.min_values.interactions =
                    min_max_vals.min_values.interactions.min(interactions);
                min_max_vals.max_values.interactions =
                    min_max_vals.max_values.interactions.max(interactions);
            }
            if let Some(inbound_bandwidth) = m.inbound_bandwidth {
                min_max_vals.min_values.inbound_bandwidth = min_max_vals
                    .min_values
                    .inbound_bandwidth
                    .min(inbound_bandwidth);
                min_max_vals.max_values.inbound_bandwidth = min_max_vals
                    .max_values
                    .inbound_bandwidth
                    .max(inbound_bandwidth);
            }
            if let Some(outbound_bandwidth) = m.outbound_bandwidth {
                min_max_vals.min_values.outbound_bandwidth = min_max_vals
                    .min_values
                    .outbound_bandwidth
                    .min(outbound_bandwidth);
                min_max_vals.max_values.outbound_bandwidth = min_max_vals
                    .max_values
                    .outbound_bandwidth
                    .max(outbound_bandwidth);
            }
            if let Some(bytes_received) = m.bytes_received {
                min_max_vals.min_values.bytes_received =
                    min_max_vals.min_values.bytes_received.min(bytes_received);
                min_max_vals.max_values.bytes_received =
                    min_max_vals.max_values.bytes_received.max(bytes_received);
            }
            if let Some(bytes_sent) = m.bytes_sent {
                min_max_vals.min_values.bytes_sent =
                    min_max_vals.min_values.bytes_sent.min(bytes_sent);
                min_max_vals.max_values.bytes_sent =
                    min_max_vals.max_values.bytes_sent.max(bytes_sent);
            }
        });
        min_max_vals
    }
}

impl From<CollectedMeasurements> for NormalizedMeasurements {
    fn from(collected_measurements: CollectedMeasurements) -> Self {
        let latency =
            statistics::calculate_z_normalized_weighted_mean(collected_measurements.latency);
        let interactions =
            statistics::calculate_z_normalized_weighted_mean(collected_measurements.interactions);
        let inbound_bandwidth = statistics::calculate_z_normalized_weighted_mean(
            collected_measurements.inbound_bandwidth,
        );
        let outbound_bandwidth = statistics::calculate_z_normalized_weighted_mean(
            collected_measurements.outbound_bandwidth,
        );
        let bytes_received =
            statistics::calculate_z_normalized_weighted_mean(collected_measurements.bytes_received);
        let bytes_sent =
            statistics::calculate_z_normalized_weighted_mean(collected_measurements.bytes_sent);
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
    pub fn min_max_normalize(&mut self, min_max_vals: MinMaxValues) {
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

    pub fn calculate_score(&self) -> Option<u8> {
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
