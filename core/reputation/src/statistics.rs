pub fn min_max_normalize_value(value: f64, min_val: f64, max_val: f64) -> f64 {
    (value - min_val) / (max_val - min_val)
}

pub fn calculate_normalized_mean(mut values: Vec<f64>) -> f64 {
    z_score_normalize_filter(&mut values);
    calculate_mean(&values).unwrap()
}

fn calculate_mean(values: &[f64]) -> Option<f64> {
    if !values.is_empty() {
        let sum: f64 = values.iter().sum();
        let n = values.len() as f64;
        Some(sum / n)
    } else {
        None
    }
}

fn calculate_variance(values: &[f64]) -> Option<f64> {
    calculate_mean(values).map(|values_mean| {
        let n = values.len() as f64;
        values
            .iter()
            .map(|v| (v - values_mean).powi(2))
            .sum::<f64>()
            / (n - 1.0)
    })
}

fn calculate_std_dev(values: &[f64]) -> Option<f64> {
    calculate_variance(values).map(|variance| variance.sqrt())
}

fn calculate_z_score(x: f64, mean: f64, std_dev: f64) -> f64 {
    ((x - mean) / (std_dev + f64::EPSILON)).abs()
}

fn z_score_normalize_filter(values: &mut Vec<f64>) {
    let std_dev = calculate_std_dev(values).unwrap();
    if std_dev.abs() > f64::EPSILON {
        let mean = calculate_mean(values).unwrap();
        values.retain(|&x| calculate_z_score(x, mean, std_dev) < 3.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let values = [];
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
        let values = [];
        assert!(calculate_variance(&values).is_none());
    }

    #[test]
    fn test_std_dev_basic() {
        let values = [2.0, 4.0, 6.0];
        let std_dev = calculate_std_dev(&values).unwrap();
        assert_eq!(std_dev, 2.0);
    }

    #[test]
    fn test_std_dev() {
        let values = [
            7777777777.0,
            554343434.0,
            123.0,
            3434343434.0,
            9988776661.0,
            6543.0,
        ];
        let std_dev = calculate_std_dev(&values).unwrap();
        assert!((std_dev - 4324111363.27575).abs() <= f64::EPSILON);
    }

    #[test]
    fn test_std_dev_empty() {
        let values = [];
        assert!(calculate_std_dev(&values).is_none());
    }
}
