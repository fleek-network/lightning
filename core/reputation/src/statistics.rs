use std::ops::{Add, Div, Mul, Sub};

use hp_fixed::signed::HpFixed;

use crate::types::WeightedValue;
use crate::PRECISION;

const EPSILON: f64 = 1e-8;

pub fn approx_quantile<T: Ord + PartialOrd + Copy>(mut values: Vec<T>, q: f64) -> Option<T> {
    if (0.0..=0.99).contains(&q) && !values.is_empty() {
        values.sort();
        let index = (q * values.len() as f64).floor() as usize;
        Some(values[index])
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
    if a > b { a - b } else { b - a }
}

fn calculate_mean<T>(values: &[T]) -> Option<T>
where
    T: Default + Add<T, Output = T> + Div<Output = T> + From<f64> + Mul<T, Output = T> + Clone,
{
    if !values.is_empty() {
        let sum = values.iter().cloned().fold(T::default(), |acc, x| acc + x);
        let n = values.len() as f64;
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
        + From<f64>
        + Sub<T, Output = T>
        + Clone,
{
    if values.len() == 1 {
        return Some(1.0.into());
    }
    calculate_mean(values).map(|values_mean| {
        let n: T = (values.len() as f64).into();
        values
            .iter()
            .cloned()
            .map(|v| (v.clone() - values_mean.clone()) * (v - values_mean.clone()))
            .fold(T::default(), |acc, x| acc + x)
            / (n - 1.0.into())
    })
}

fn calculate_z_score<T>(x: T, mean: T, variance: T) -> T
where
    T: Add<T, Output = T>
        + Mul<T, Output = T>
        + From<f64>
        + Sub<T, Output = T>
        + Div<T, Output = T>
        + Clone,
{
    ((x.clone() - mean.clone()) * (x - mean)) / (variance + EPSILON.into())
}

fn z_score_normalize_filter<T>(values: &mut Vec<T>)
where
    T: Default
        + Add<T, Output = T>
        + Div<Output = T>
        + Mul<T, Output = T>
        + From<f64>
        + Sub<T, Output = T>
        + PartialOrd<T>
        + Clone,
{
    if let Some(variance) = calculate_variance(values) {
        if variance > EPSILON.into() {
            if let Some(mean) = calculate_mean(values) {
                values.retain(|x| {
                    calculate_z_score(x.clone(), mean.clone(), variance.clone()) < 9.0.into()
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
        + From<f64>
        + Mul<T, Output = T>
        + Clone,
{
    if !values.is_empty() {
        let mean = values
            .iter()
            .cloned()
            .fold(0.0.into(), |acc: HpFixed<PRECISION>, v| {
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
        + From<f64>
        + From<usize>
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
        let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let mean = calculate_mean(&values).unwrap();
        assert_eq!(mean, 5.0);
    }

    #[test]
    fn test_mean() {
        let values = [123134134.0, 4134.0, 12312.0, 999999999.0, 232323232323.0];
        let mean = calculate_mean(&values).unwrap();
        assert!((mean - 46689276580.4).abs() <= f64::EPSILON);
    }

    #[test]
    fn test_mean_one() {
        let values = [42.0];
        assert_eq!(calculate_mean(&values).unwrap(), 42.0);
    }

    #[test]
    fn test_mean_empty() {
        let values: Vec<f64> = vec![];
        assert!(calculate_mean(&values).is_none());
    }

    #[test]
    fn test_variance_basic() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let var = calculate_variance(&values).unwrap();
        assert_eq!(var, 6.0);
    }

    #[test]
    fn test_variance() {
        let values = [
            999999999999.0,
            5738389400.0,
            123124134343.0,
            413434009984.0,
            1.0,
            12323.0,
        ];
        let var = calculate_variance(&values).unwrap();
        assert!((var - 157934744570755219980288.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn test_variance_empty() {
        let values: Vec<f64> = vec![];
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
        assert_eq!(approx_quantile(values, 0.1).unwrap(), 2);
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(approx_quantile(values, 0.0).unwrap(), 1);
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(approx_quantile(values, 1.0), None);
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(approx_quantile(values, 0.9999), None);
        let values: Vec<u32> = Vec::new();
        assert_eq!(approx_quantile(values, 0.5), None);
    }

    #[test]
    fn test_z_score_normalize_filter() {
        let mut values = vec![
            1.0, 4.0, 6.0, 3.0, 4.0, 7.0, 4.0, 5.0, 6.0, 7.0, 8.0, 3.0, 80.0,
        ];
        z_score_normalize_filter(&mut values);
        assert_eq!(
            values,
            vec![1.0, 4.0, 6.0, 3.0, 4.0, 7.0, 4.0, 5.0, 6.0, 7.0, 8.0, 3.0]
        )
    }

    #[test]
    fn test_z_score_normalize_filter_empty() {
        let mut values: Vec<f64> = vec![];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, Vec::<f64>::new())
    }

    #[test]
    fn test_z_score_normalize_filter_one_element() {
        let mut values = vec![1.0];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, vec![1.0])
    }

    #[test]
    fn test_z_score_normalize_filter_two_elements() {
        let mut values = vec![1.0, 2.0];
        z_score_normalize_filter(&mut values);
        assert_eq!(values, vec![1.0, 2.0])
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
        let mut values = vec![4.0, 6.0, 3.0, 4.0, 5.0, 2.0, 2.0, 1.0, 3.0, 2.0, 4.0, 50.0];
        z_score_normalize_filter(&mut values);
        assert_eq!(
            values,
            vec![4.0, 6.0, 3.0, 4.0, 5.0, 2.0, 2.0, 1.0, 3.0, 2.0, 4.0]
        );
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
