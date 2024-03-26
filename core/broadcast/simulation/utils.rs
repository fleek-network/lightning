use std::collections::BTreeMap;

use fxhash::FxHashMap;

#[derive(Debug, Clone)]
pub struct Summary {
    pub mean: f64,
    pub variance: f64,
    pub n: usize,
}

#[allow(unused)]
pub fn get_mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    let sum = data.iter().sum::<f64>();
    Some(sum / data.len() as f64)
}

#[allow(unused)]
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

#[allow(unused)]
pub fn get_nodes_reached_per_timestep(
    emitted: &FxHashMap<String, FxHashMap<u128, u32>>,
    num_nodes_total: usize,
    cumulative: bool,
    precision_in_ms: u128,
) -> BTreeMap<u128, Vec<f64>> {
    let mut steps_to_num_nodes = BTreeMap::<u128, Vec<f64>>::new();
    for timesteps in emitted.values() {
        let min_step = *timesteps.keys().min().unwrap();
        let max_step = *timesteps.keys().max().unwrap();

        let mut sum = 0.0;
        for step in min_step..(max_step + 1) {
            let num_nodes = timesteps.get(&step).unwrap_or(&0);
            let perc = (*num_nodes) as f64 / num_nodes_total as f64;
            let value = if cumulative {
                sum += perc;
                sum
            } else {
                perc
            };
            let normalized_step = step - min_step;
            let normalized_step = (normalized_step / precision_in_ms) * precision_in_ms;
            steps_to_num_nodes
                .entry(normalized_step)
                .or_default()
                .push(value);
        }
    }
    steps_to_num_nodes
}

#[allow(unused)]
pub fn get_nodes_reached_per_timestep_summary(
    timesteps_to_num_nodes: &BTreeMap<u128, Vec<f64>>,
) -> Vec<Summary> {
    let mut steps_to_num_nodes_mean_var = Vec::new();

    for num_nodes in timesteps_to_num_nodes.values() {
        let mean = get_mean(num_nodes).unwrap();
        let variance = get_variance(num_nodes).unwrap();
        let summary = Summary {
            mean,
            variance,
            n: num_nodes.len(),
        };
        steps_to_num_nodes_mean_var.push(summary);
    }
    steps_to_num_nodes_mean_var
}
