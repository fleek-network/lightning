use std::{
    collections::HashMap,
    ops::{Add, Div, Mul, Sub},
};

use fleek_crypto::NodePublicKey;
use freek_interfaces::types::ReputationMeasurements;
use hp_fixed::signed::HpFixed;

use crate::{statistics, PRECISION};

pub trait WeightedValue {
    fn get_weighted_value(&self) -> HpFixed<PRECISION>;
}

#[derive(Debug, Clone)]
pub(crate) struct WeightedFloat {
    pub(crate) value: HpFixed<PRECISION>,
    pub(crate) weight: HpFixed<PRECISION>,
}

impl From<usize> for WeightedFloat {
    fn from(value: usize) -> Self {
        WeightedFloat {
            value: (value as i128).into(),
            weight: 1.0.into(),
        }
    }
}

impl WeightedValue for WeightedFloat {
    fn get_weighted_value(&self) -> HpFixed<PRECISION> {
        self.value.clone() * self.weight.clone()
    }
}

impl Default for WeightedFloat {
    fn default() -> Self {
        WeightedFloat {
            value: 0.0.into(),
            weight: 1.0.into(),
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
        WeightedFloat {
            value: value.into(),
            weight: 1.0.into(),
        }
    }
}

impl From<HpFixed<PRECISION>> for WeightedFloat {
    fn from(value: HpFixed<PRECISION>) -> Self {
        WeightedFloat {
            value,
            weight: 1.0.into(),
        }
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
    pub latency: Option<HpFixed<PRECISION>>,
    pub interactions: Option<HpFixed<PRECISION>>,
    pub inbound_bandwidth: Option<HpFixed<PRECISION>>,
    pub outbound_bandwidth: Option<HpFixed<PRECISION>>,
    pub bytes_received: Option<HpFixed<PRECISION>>,
    pub bytes_sent: Option<HpFixed<PRECISION>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Values {
    latency: HpFixed<PRECISION>,
    interactions: HpFixed<PRECISION>,
    inbound_bandwidth: HpFixed<PRECISION>,
    outbound_bandwidth: HpFixed<PRECISION>,
    bytes_received: HpFixed<PRECISION>,
    bytes_sent: HpFixed<PRECISION>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MinMaxValues {
    min_values: Values,
    max_values: Values,
}

impl From<Vec<WeightedReputationMeasurements>> for CollectedMeasurements {
    fn from(weighted_measurements: Vec<WeightedReputationMeasurements>) -> Self {
        let mut weight_sum_latency: HpFixed<PRECISION> = 0.0.into();
        let mut weight_sum_interactions: HpFixed<PRECISION> = 0.0.into();
        let mut weight_sum_inbound_bandwidth: HpFixed<PRECISION> = 0.0.into();
        let mut weight_sum_outbound_bandwidth: HpFixed<PRECISION> = 0.0.into();
        let mut weight_sum_bytes_received: HpFixed<PRECISION> = 0.0.into();
        let mut weight_sum_bytes_sent: HpFixed<PRECISION> = 0.0.into();

        let mut count_latency = 0;
        let mut count_interactions = 0;
        let mut count_inbound_bandwidth = 0;
        let mut count_outbound_bandwidth = 0;
        let mut count_bytes_received = 0;
        let mut count_bytes_sent = 0;
        weighted_measurements.iter().for_each(|m| {
            if m.measurements.latency.is_some() {
                weight_sum_latency += (m.weight as i16).into();
                count_latency += 1;
            }
            if m.measurements.interactions.is_some() {
                weight_sum_interactions += (m.weight as i16).into();
                count_interactions += 1;
            }
            if m.measurements.inbound_bandwidth.is_some() {
                weight_sum_inbound_bandwidth += (m.weight as i16).into();
                count_inbound_bandwidth += 1;
            }
            if m.measurements.outbound_bandwidth.is_some() {
                weight_sum_outbound_bandwidth += (m.weight as i16).into();
                count_outbound_bandwidth += 1;
            }
            if m.measurements.bytes_received.is_some() {
                weight_sum_bytes_received += (m.weight as i16).into();
                count_bytes_received += 1;
            }
            if m.measurements.bytes_sent.is_some() {
                weight_sum_bytes_sent += (m.weight as i16).into();
                count_bytes_sent += 1;
            }
        });
        let mut measurements = Self::default();
        weighted_measurements.into_iter().for_each(|m| {
            if let Some(latency) = m.measurements.latency {
                let weight = if weight_sum_latency == 0.into() {
                    HpFixed::<PRECISION>::from(1) / HpFixed::<PRECISION>::from(count_latency)
                } else {
                    HpFixed::<PRECISION>::from(m.weight as i16) / weight_sum_latency.clone()
                };
                measurements.latency.push(WeightedFloat {
                    value: i128::try_from(latency.as_millis())
                        .unwrap_or(i128::MAX)
                        .into(),
                    weight,
                });
            }
            if let Some(interactions) = m.measurements.interactions {
                let weight = if weight_sum_interactions == 0.into() {
                    HpFixed::<PRECISION>::from(1) / HpFixed::<PRECISION>::from(count_interactions)
                } else {
                    HpFixed::<PRECISION>::from(m.weight as i16) / weight_sum_interactions.clone()
                };
                measurements.interactions.push(WeightedFloat {
                    value: interactions.into(),
                    weight,
                });
            }
            if let Some(inbound_bandwidth) = m.measurements.inbound_bandwidth {
                let weight = if weight_sum_inbound_bandwidth == 0.into() {
                    HpFixed::<PRECISION>::from(1)
                        / HpFixed::<PRECISION>::from(count_inbound_bandwidth)
                } else {
                    HpFixed::<PRECISION>::from(m.weight as i16)
                        / weight_sum_inbound_bandwidth.clone()
                };
                measurements.inbound_bandwidth.push(WeightedFloat {
                    value: i128::try_from(inbound_bandwidth)
                        .unwrap_or(i128::MAX)
                        .into(),
                    weight,
                });
            }
            if let Some(outbound_bandwidth) = m.measurements.outbound_bandwidth {
                let weight = if weight_sum_outbound_bandwidth == 0.into() {
                    HpFixed::<PRECISION>::from(1)
                        / HpFixed::<PRECISION>::from(count_outbound_bandwidth)
                } else {
                    HpFixed::<PRECISION>::from(m.weight as i16)
                        / weight_sum_outbound_bandwidth.clone()
                };
                measurements.outbound_bandwidth.push(WeightedFloat {
                    value: i128::try_from(outbound_bandwidth)
                        .unwrap_or(i128::MAX)
                        .into(),
                    weight,
                });
            }
            if let Some(bytes_received) = m.measurements.bytes_received {
                let weight = if weight_sum_bytes_received == 0.into() {
                    HpFixed::<PRECISION>::from(1) / HpFixed::<PRECISION>::from(count_bytes_received)
                } else {
                    HpFixed::<PRECISION>::from(m.weight as i16) / weight_sum_bytes_received.clone()
                };
                measurements.bytes_received.push(WeightedFloat {
                    value: i128::try_from(bytes_received).unwrap_or(i128::MAX).into(),
                    weight,
                });
            }
            if let Some(bytes_sent) = m.measurements.bytes_sent {
                let weight = if weight_sum_bytes_sent == 0.into() {
                    HpFixed::<PRECISION>::from(1) / HpFixed::<PRECISION>::from(count_bytes_sent)
                } else {
                    HpFixed::<PRECISION>::from(m.weight as i16) / weight_sum_bytes_sent.clone()
                };
                measurements.bytes_sent.push(WeightedFloat {
                    value: i128::try_from(bytes_sent).unwrap_or(i128::MAX).into(),
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
        if weight_sum_latency != 0.into() && measurements.latency.len() > 1 {
            let inverse_weight_sum = measurements
                .latency
                .iter()
                .fold(HpFixed::<PRECISION>::from(0), |acc, w| {
                    acc + (HpFixed::<PRECISION>::from(1) - w.weight.clone())
                });
            measurements.latency.iter_mut().for_each(|w| {
                w.weight =
                    (HpFixed::<PRECISION>::from(1) - w.weight.clone()) / inverse_weight_sum.clone();
            });
        }
        measurements
    }
}

impl From<&HashMap<NodePublicKey, NormalizedMeasurements>> for MinMaxValues {
    fn from(normalized_measurements: &HashMap<NodePublicKey, NormalizedMeasurements>) -> Self {
        let min_values = Values {
            latency: HpFixed::<PRECISION>::from(f64::MAX),
            interactions: HpFixed::<PRECISION>::from(f64::MAX),
            inbound_bandwidth: HpFixed::<PRECISION>::from(f64::MAX),
            outbound_bandwidth: HpFixed::<PRECISION>::from(f64::MAX),
            bytes_received: HpFixed::<PRECISION>::from(f64::MAX),
            bytes_sent: HpFixed::<PRECISION>::from(f64::MAX),
        };
        let max_values = Values {
            latency: HpFixed::<PRECISION>::from(f64::MIN),
            interactions: HpFixed::<PRECISION>::from(f64::MIN),
            inbound_bandwidth: HpFixed::<PRECISION>::from(f64::MIN),
            outbound_bandwidth: HpFixed::<PRECISION>::from(f64::MIN),
            bytes_received: HpFixed::<PRECISION>::from(f64::MIN),
            bytes_sent: HpFixed::<PRECISION>::from(f64::MIN),
        };
        let mut min_max_vals = MinMaxValues {
            min_values,
            max_values,
        };

        normalized_measurements.iter().for_each(|(_, m)| {
            if let Some(latency) = &m.latency {
                min_max_vals.min_values.latency =
                    min_max_vals.min_values.latency.clone().min(latency.clone());
                min_max_vals.max_values.latency =
                    min_max_vals.max_values.latency.clone().max(latency.clone());
            }
            if let Some(interactions) = &m.interactions {
                min_max_vals.min_values.interactions = min_max_vals
                    .min_values
                    .interactions
                    .clone()
                    .min(interactions.clone());
                min_max_vals.max_values.interactions = min_max_vals
                    .max_values
                    .interactions
                    .clone()
                    .max(interactions.clone());
            }
            if let Some(inbound_bandwidth) = &m.inbound_bandwidth {
                min_max_vals.min_values.inbound_bandwidth = min_max_vals
                    .min_values
                    .inbound_bandwidth
                    .clone()
                    .min(inbound_bandwidth.clone());
                min_max_vals.max_values.inbound_bandwidth = min_max_vals
                    .max_values
                    .inbound_bandwidth
                    .clone()
                    .max(inbound_bandwidth.clone());
            }
            if let Some(outbound_bandwidth) = &m.outbound_bandwidth {
                min_max_vals.min_values.outbound_bandwidth = min_max_vals
                    .min_values
                    .outbound_bandwidth
                    .clone()
                    .min(outbound_bandwidth.clone());
                min_max_vals.max_values.outbound_bandwidth = min_max_vals
                    .max_values
                    .outbound_bandwidth
                    .clone()
                    .max(outbound_bandwidth.clone());
            }
            if let Some(bytes_received) = &m.bytes_received {
                min_max_vals.min_values.bytes_received = min_max_vals
                    .min_values
                    .bytes_received
                    .clone()
                    .min(bytes_received.clone());
                min_max_vals.max_values.bytes_received = min_max_vals
                    .max_values
                    .bytes_received
                    .clone()
                    .max(bytes_received.clone());
            }
            if let Some(bytes_sent) = &m.bytes_sent {
                min_max_vals.min_values.bytes_sent = min_max_vals
                    .min_values
                    .bytes_sent
                    .clone()
                    .min(bytes_sent.clone());
                min_max_vals.max_values.bytes_sent = min_max_vals
                    .max_values
                    .bytes_sent
                    .clone()
                    .max(bytes_sent.clone());
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
        if let Some(latency) = self.latency.clone() {
            self.latency = statistics::try_min_max_normalize(
                latency,
                min_max_vals.min_values.latency,
                min_max_vals.max_values.latency,
            );
        }
        if let Some(interactions) = self.interactions.clone() {
            self.interactions = statistics::try_min_max_normalize(
                interactions,
                min_max_vals.min_values.interactions,
                min_max_vals.max_values.interactions,
            );
        }
        if let Some(inbound_bandwidth) = self.inbound_bandwidth.clone() {
            self.inbound_bandwidth = statistics::try_min_max_normalize(
                inbound_bandwidth,
                min_max_vals.min_values.inbound_bandwidth,
                min_max_vals.max_values.inbound_bandwidth,
            );
        }
        if let Some(outbound_bandwidth) = self.outbound_bandwidth.clone() {
            self.outbound_bandwidth = statistics::try_min_max_normalize(
                outbound_bandwidth,
                min_max_vals.min_values.outbound_bandwidth,
                min_max_vals.max_values.outbound_bandwidth,
            );
        }
        if let Some(bytes_received) = self.bytes_received.clone() {
            self.bytes_received = statistics::try_min_max_normalize(
                bytes_received,
                min_max_vals.min_values.bytes_received,
                min_max_vals.max_values.bytes_received,
            );
        }
        if let Some(bytes_sent) = self.bytes_sent.clone() {
            self.bytes_sent = statistics::try_min_max_normalize(
                bytes_sent,
                min_max_vals.min_values.bytes_sent,
                min_max_vals.max_values.bytes_sent,
            );
        }
    }

    pub fn calculate_score(&self) -> Option<u8> {
        let mut score = HpFixed::<PRECISION>::from(0);
        let mut count = 0;
        if let Some(latency) = &self.latency {
            score += HpFixed::<PRECISION>::from(1) - latency;
            count += 1;
        }
        if let Some(interactions) = &self.interactions {
            score = score + interactions;
            count += 1;
        }
        if let Some(inbound_bandwidth) = &self.inbound_bandwidth {
            score = score + inbound_bandwidth;
            count += 1;
        }
        if let Some(outbound_bandwidth) = &self.outbound_bandwidth {
            score = score + outbound_bandwidth;
            count += 1;
        }
        if let Some(bytes_received) = &self.bytes_received {
            score = score + bytes_received;
            count += 1;
        }
        if let Some(bytes_sent) = &self.bytes_sent {
            score = score + bytes_sent;
            count += 1;
        }

        if count == 0 {
            return None;
        }
        // This value will be in the range [0, 1]
        score = score / HpFixed::<PRECISION>::from(count);
        let score: i128 = (score * HpFixed::<PRECISION>::from(100)).try_into().ok()?;
        // The value of score will be in range [0, 100]
        Some(score.min(100) as u8)
    }
}
