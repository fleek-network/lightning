use std::ops::{Add, Div, Mul, Sub};

use hp_fixed::signed::HpFixed;
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::PRECISION;

use crate::types::WeightedValue;

const EPSILON: f64 = 1e-8;

pub fn approx_quantile<T: Ord + PartialOrd + Copy>(
    mut values: Vec<T>,
    q: HpUfixed<PRECISION>,
) -> Option<T> {
    if q >= HpUfixed::<PRECISION>::from(0.0)
        && q <= HpUfixed::<PRECISION>::from(0.99)
        && !values.is_empty()
    {
        values.sort();
        let index = q * HpUfixed::<PRECISION>::from(values.len());
        let Ok(index) = u128::try_from(index.floor()) else {
            return None;
        };
        Some(values[index as usize])
    } else {
        None
    }
}

pub fn try_min_max_normalize<T>(value: T, min_value: T, max_value: T) -> Option<T>
where
    T: Sub<T, Output = T> + PartialOrd<T> + Div<Output = T> + From<f64> + Clone,
{
    if abs_difference(min_value.clone(), max_value.clone()) < EPSILON.into() {
        return None;
    }
    Some(min_max_normalize(value, min_value, max_value))
}

pub fn min_max_normalize<T>(value: T, min_value: T, max_value: T) -> T
where
    T: Sub<T, Output = T> + Div<Output = T> + Clone,
{
    (value - min_value.clone()) / (max_value - min_value)
}

fn abs_difference<T>(a: T, b: T) -> T
where
    T: Sub<T, Output = T> + PartialOrd<T>,
{
    if a > b {
        a - b
    } else {
        b - a
    }
}

fn calculate_mean<T>(values: &[T]) -> Option<T>
where
    T: Default + Add<T, Output = T> + Div<Output = T> + From<i128> + Mul<T, Output = T> + Clone,
{
    if !values.is_empty() {
        let sum = values.iter().cloned().fold(T::default(), |acc, x| acc + x);
        let n = values.len() as i128;
        Some(sum / n.into())
    } else {
        None
    }
}

fn calculate_variance<T>(values: &[T]) -> Option<T>
where
    T: Default
        + Add<T, Output = T>
        + Div<Output = T>
        + Mul<T, Output = T>
        + From<i128>
        + Sub<T, Output = T>
        + Clone,
{
    if values.len() == 1 {
        return Some(1.into());
    }
    calculate_mean(values).map(|values_mean| {
        let n: T = (values.len() as i128).into();
        values
            .iter()
            .cloned()
            .map(|v| (v.clone() - values_mean.clone()) * (v - values_mean.clone()))
            .fold(T::default(), |acc, x| acc + x)
            / (n - 1.into())
    })
}

fn calculate_z_score<T>(x: T, mean: T, variance: T) -> T
where
    T: Add<T, Output = T>
        + Mul<T, Output = T>
        + From<i128>
        + Sub<T, Output = T>
        + Div<T, Output = T>
        + Clone,
{
    ((x.clone() - mean.clone()) * (x - mean)) / variance
}

fn z_score_normalize_filter<T>(values: &mut Vec<T>)
where
    T: Default
        + Add<T, Output = T>
        + Div<Output = T>
        + Mul<T, Output = T>
        + From<i128>
        + Sub<T, Output = T>
        + PartialOrd<T>
        + Clone,
{
    if let Some(variance) = calculate_variance(values) {
        if variance > 0.into() {
            if let Some(mean) = calculate_mean(values) {
                values.retain(|x| {
                    calculate_z_score(x.clone(), mean.clone(), variance.clone()) < 9.into()
                });
            }
        }
    }
}

fn calculate_weighted_mean<T>(values: &[T]) -> Option<HpFixed<PRECISION>>
where
    T: Default
        + WeightedValue
        + Add<T, Output = T>
        + Div<Output = T>
        + From<i128>
        + Mul<T, Output = T>
        + Clone,
{
    if !values.is_empty() {
        let mean = values
            .iter()
            .cloned()
            .fold(0.into(), |acc: HpFixed<PRECISION>, v| {
                acc + v.get_weighted_value()
            });
        Some(mean)
    } else {
        None
    }
}

pub fn calculate_z_normalized_weighted_mean<T>(mut values: Vec<T>) -> Option<HpFixed<PRECISION>>
where
    T: Default
        + WeightedValue
        + Add<T, Output = T>
        + Div<Output = T>
        + Mul<T, Output = T>
        + From<i128>
        + Sub<T, Output = T>
        + PartialOrd<T>
        + Clone,
{
    z_score_normalize_filter(&mut values);
    calculate_weighted_mean(&values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WeightedFloat;

    #[test]
    fn test_mean_basic() {
        let values = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mean = calculate_mean(&values).unwrap();
        assert_eq!(mean, 5);
    }

    #[test]
    fn test_mean() {
        let values = [
            HpFixed::<18>::from(123134134_i64),
            HpFixed::<18>::from(4134_i64),
            HpFixed::<18>::from(12312_i64),
            HpFixed::<18>::from(999999999_i64),
            HpFixed::<18>::from(232323232323_i64),
        ];
        let mean = calculate_mean(&values).unwrap();
        //assert!((mean - HpUfixed::<18>::from(46689276580.4)).abs() <= f64::EPSILON);
        assert_eq!(mean, HpFixed::<18>::from(46689276580.4))
    }

    #[test]
    fn test_mean_one() {
        let values = [HpFixed::<18>::from(42.0)];
        assert_eq!(calculate_mean(&values).unwrap(), HpFixed::<18>::from(42.0));
    }

    #[test]
    fn test_mean_empty() {
        let values: Vec<i128> = vec![];
        assert!(calculate_mean(&values).is_none());
    }

    #[test]
    fn test_variance_basic() {
        let values = [
            HpFixed::<18>::from(1_i64),
            HpFixed::<18>::from(2_i64),
            HpFixed::<18>::from(3_i64),
            HpFixed::<18>::from(4_i64),
            HpFixed::<18>::from(5_i64),
            HpFixed::<18>::from(6_i64),
            HpFixed::<18>::from(7_i64),
            HpFixed::<18>::from(8_i64),
        ];
        let var = calculate_variance(&values).unwrap();
        assert_eq!(var, HpFixed::<18>::from(6));
    }

    #[test]
    fn test_variance() {
        let values = [
            HpFixed::<32>::from(57389400_i128),
            HpFixed::<32>::from(123124143_i128),
            HpFixed::<32>::from(413434984_i128),
            HpFixed::<32>::from(1_i128),
        ];
        let var = calculate_variance(&values).unwrap();
        // TODO(matthias): find out why its rounding here
        //assert_eq!(var, HpFixed::<32>::from(33729290112861004_i128));
        assert!(HpFixed::<32>::from(33729290112861004_i128) - var < HpFixed::<32>::from(100_i128));
    }

    #[test]
    fn test_variance_empty() {
        let values: Vec<i128> = vec![];
        assert!(calculate_variance(&values).is_none());
    }

    #[test]
    fn test_min_max_normalize() {
        assert_eq!(min_max_normalize(80.0, 20.0, 100.0), 0.75);
        assert_eq!(min_max_normalize(100.0, 20.0, 100.0), 1.0);
        assert_eq!(min_max_normalize(20.0, 20.0, 100.0), 0.0);
    }

    #[test]
    fn test_try_min_max_normalize() {
        assert_eq!(try_min_max_normalize(50.0, 50.0, 50.0), None);
        assert_eq!(try_min_max_normalize(50.0, 10.0, 90.0), Some(0.5));
    }

    #[test]
    fn test_approx_quantile() {
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(
            approx_quantile(values, HpUfixed::<PRECISION>::from(0.1)).unwrap(),
            2
        );
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(
            approx_quantile(values, HpUfixed::<PRECISION>::from(0.0)).unwrap(),
            1
        );
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(
            approx_quantile(values, HpUfixed::<PRECISION>::from(1.0)),
            None
        );
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(
            approx_quantile(values, HpUfixed::<PRECISION>::from(0.9999)),
            None
        );
        let values: Vec<u32> = Vec::new();
        assert_eq!(
            approx_quantile(values, HpUfixed::<PRECISION>::from(0.5)),
            None
        );
    }

    #[test]
    fn test_z_score_normalize_filter() {
        let mut values = vec![1, 4, 6, 3, 4, 7, 4, 5, 6, 7, 8, 3, 80];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, vec![1, 4, 6, 3, 4, 7, 4, 5, 6, 7, 8, 3])
    }

    #[test]
    fn test_z_score_normalize_filter_empty() {
        let mut values: Vec<i128> = vec![];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, Vec::<i128>::new())
    }

    #[test]
    fn test_z_score_normalize_filter_one_element() {
        let mut values = vec![1];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, vec![1])
    }

    #[test]
    fn test_z_score_normalize_filter_two_elements() {
        let mut values = vec![1, 2];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, vec![1, 2])
    }

    #[test]
    fn test_calculate_weighted_mean() {
        let values = vec![
            WeightedFloat {
                value: 1.0.into(),
                weight: 0.2.into(),
            },
            WeightedFloat {
                value: 2.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 3.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 4.0.into(),
                weight: 0.3.into(),
            },
            WeightedFloat {
                value: 5.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 6.0.into(),
                weight: 0.2.into(),
            },
        ];
        let weighted_mean = calculate_weighted_mean(&values);
        assert_eq!(weighted_mean, Some(3.6.into()));
    }

    #[test]
    fn test_calculate_weighted_mean_empty() {
        let values: Vec<WeightedFloat> = vec![];
        let weighted_mean = calculate_weighted_mean(&values);
        assert_eq!(weighted_mean, None);
    }

    #[test]
    fn test_z_score_normalize_filter_more() {
        let mut values = vec![4, 6, 3, 4, 5, 2, 2, 1, 3, 2, 4, 50];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, vec![4, 6, 3, 4, 5, 2, 2, 1, 3, 2, 4]);
    }

    #[test]
    fn test_calculate_z_normalized_weighted_mean() {
        let values = vec![
            WeightedFloat {
                value: 4.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 6.0.into(),
                weight: 0.01.into(),
            },
            WeightedFloat {
                value: 3.0.into(),
                weight: 0.01.into(),
            },
            WeightedFloat {
                value: 4.0.into(),
                weight: 0.18.into(),
            },
            WeightedFloat {
                value: 5.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 2.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 2.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 1.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 3.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 2.0.into(),
                weight: 0.1.into(),
            },
            WeightedFloat {
                value: 4.0.into(),
                weight: 0.05.into(),
            },
            WeightedFloat {
                value: 50.0.into(),
                weight: 0.05.into(),
            },
        ];
        let weighted_mean = calculate_z_normalized_weighted_mean(values);
        assert_eq!(weighted_mean, Some(2.91.into()));
    }
}
