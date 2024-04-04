pub fn get_mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    let sum = data.iter().sum::<f64>();
    Some(sum / data.len() as f64)
}

pub fn get_variance(data: &[f64]) -> Option<f64> {
    match (get_mean(data), data.len()) {
        (Some(data_mean), count) if count > 0 => {
            let variance = data
                .iter()
                .map(|value| {
                    let diff = data_mean - *value;

                    diff * diff
                })
                .sum::<f64>()
                / count as f64;

            Some(variance)
        },
        _ => None,
    }
}
