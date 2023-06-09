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
