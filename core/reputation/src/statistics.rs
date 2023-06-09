pub fn min_max_normalization(
    values: &[f64],
    min_val: Option<f64>,
    max_val: Option<f64>,
) -> Vec<f64> {
    let (min_val, max_val) = if let (Some(min_val), Some(max_val)) = (min_val, max_val) {
        (min_val, max_val)
    } else {
        values
            .iter()
            .fold((f64::MAX, f64::MIN), |(min_val, max_val), x| {
                (f64::min(min_val, *x), f64::max(max_val, *x))
            })
    };
    if (min_val - max_val).abs() < f64::EPSILON {
        vec![0.0; values.len()]
    } else {
        values
            .iter()
            .map(|&x| ((x - min_val) / (max_val - min_val)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_max_normalization_empty() {
        let values = [];
        assert_eq!(min_max_normalization(&values, None, None), vec![]);
    }

    #[test]
    fn test_min_max_normalization_same() {
        let values = [1.0, 1.0, 1.0];
        assert_eq!(
            min_max_normalization(&values, None, None),
            vec![0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn test_min_max_normalization_basic() {
        let values = [1234.0, 23123.0, 1.0, 1003.0, 84624.0, 123.0];
        assert_eq!(
            min_max_normalization(&values, None, None),
            vec![
                0.014570506836202923,
                0.2732354088132068,
                0.0,
                0.011840752514091914,
                1.0,
                0.0014416884298594946
            ]
        );
    }

    #[test]
    fn test_min_max_normalization_provided() {
        let values = [1234.0, 23123.0, 1.0, 1003.0, 84624.0, 123.0];
        assert_eq!(
            min_max_normalization(&values, Some(1.0), Some(84624.0)),
            vec![
                0.014570506836202923,
                0.2732354088132068,
                0.0,
                0.011840752514091914,
                1.0,
                0.0014416884298594946
            ]
        );
    }
}
