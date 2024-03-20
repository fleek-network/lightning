use std::collections::BTreeMap;

use fxhash::FxHashMap;
use plotters::prelude::*;

fn get_mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    let sum = data.iter().sum::<f64>();
    Some(sum / data.len() as f64)
}

fn get_std_dev(data: &[f64]) -> Option<f64> {
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

            Some(variance.sqrt())
        },
        _ => None,
    }
}

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

pub fn get_nodes_reached_per_timestep_summary(
    timesteps_to_num_nodes: &BTreeMap<u128, Vec<f64>>,
) -> Vec<(i32, i32)> {
    let mut steps_to_num_nodes_mean_var = Vec::new();

    for num_nodes in timesteps_to_num_nodes.values() {
        let mean = get_mean(num_nodes).unwrap();
        let std_dev = get_std_dev(num_nodes).unwrap();
        steps_to_num_nodes_mean_var.push(((mean * 1000.) as i32, (std_dev * 1000.) as i32));
    }
    steps_to_num_nodes_mean_var
}

#[allow(clippy::too_many_arguments)]
pub fn plot_bar_chart(
    data: Vec<(i32, i32)>, // (mean, std_dev)
    title: &str,
    x_label: &str,
    y_label: &str,
    color: RGBColor,
    error_bars: bool,
    output_path: &std::path::Path,
) {
    if let Some(directory) = output_path.parent() {
        if !directory.exists() {
            std::fs::create_dir_all(directory).expect("Failed to create directory");
        }
    }
    let (means, std_devs): (Vec<_>, Vec<_>) = data.into_iter().unzip();

    let max_x = means.len();
    let max_y = 1000;

    let root_area = BitMapBackend::new(output_path, (1000, 800)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    fn y_label_fmt(x: &i32) -> String {
        format!("{}", x / 10)
    }

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 80)
        .set_label_area_size(LabelAreaPosition::Bottom, 60)
        .caption(title, ("sans-serif", 35))
        .build_cartesian_2d((0..max_x).into_segmented(), 0..max_y)
        .unwrap();

    ctx.configure_mesh()
        .y_label_formatter(&y_label_fmt)
        .x_desc(x_label)
        .y_desc(y_label)
        .x_label_style(("sans-serif", 25))
        .y_label_style(("sans-serif", 25))
        .draw()
        .unwrap();

    ctx.draw_series((0..).zip(means.iter()).map(|(x, y)| {
        let x0 = SegmentValue::Exact(x);
        let x1 = SegmentValue::Exact(x + 1);
        let mut bar = Rectangle::new([(x0, 0), (x1, *y)], color.filled());
        bar.set_margin(0, 0, 5, 5);
        bar
    }))
    .unwrap();

    if error_bars {
        let len = means.len() as i32;
        let mean_std_dev = means.into_iter().zip(std_devs);
        ctx.draw_series((0..len).zip(mean_std_dev).map(|(x, (m, s))| {
            ErrorBar::new_vertical(
                SegmentValue::CenterOf(x as usize),
                m - s,
                m,
                m + s,
                BLACK.filled(),
                10,
            )
        }))
        .unwrap();
    }
}
