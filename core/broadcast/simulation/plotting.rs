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
) -> FxHashMap<u128, Vec<f64>> {
    let mut steps_to_num_nodes = FxHashMap::<u128, Vec<f64>>::default();
    for timesteps in emitted.values() {
        let min_step = *timesteps.keys().min().unwrap();

        let mut steps: Vec<u128> = timesteps.keys().copied().collect();
        if cumulative {
            // processing the steps in order is only necessary for the cumulative plot
            steps.sort();
        }

        let mut sum = 0.0;
        for step in steps {
            let num_nodes = timesteps.get(&step).unwrap();
            let perc = (*num_nodes) as f64 / num_nodes_total as f64;
            let value = if cumulative {
                sum += perc;
                sum
            } else {
                perc
            };
            steps_to_num_nodes
                .entry(step - min_step)
                .or_default()
                .push(value);
        }
    }
    steps_to_num_nodes
}

pub fn get_nodes_reached_per_timestep_summary(
    timesteps_to_num_nodes: &FxHashMap<u128, Vec<f64>>,
) -> Vec<(i32, i32)> {
    let mut steps_to_num_nodes_mean_var = Vec::new();

    let mut num_taken = 0;
    let mut index = 0;
    while num_taken < timesteps_to_num_nodes.len() {
        if let Some(num_nodes) = timesteps_to_num_nodes.get(&index) {
            num_taken += 1;
            let mean = get_mean(num_nodes).unwrap();
            let std_dev = get_std_dev(num_nodes).unwrap();
            steps_to_num_nodes_mean_var.push(((mean * 1000.) as i32, (std_dev * 1000.) as i32));
        } else {
            steps_to_num_nodes_mean_var.push((0, 0));
        }
        index += 1;
    }
    steps_to_num_nodes_mean_var
}

#[allow(clippy::too_many_arguments)]
pub fn plot_bar_chart(
    data: Vec<(i32, i32)>, // (mean, std_dev)
    precision_in_ms: usize,
    title: &str,
    x_label: &str,
    y_label: &str,
    color: RGBColor,
    error_bars: bool,
    output_path: &std::path::Path,
) {
    let (means, std_devs): (Vec<_>, Vec<_>) = data.into_iter().unzip();
    let means: Vec<i32> = means
        .chunks(precision_in_ms)
        .map(|chunk| chunk.iter().sum())
        .collect();

    let std_devs: Vec<i32> = std_devs
        .chunks(precision_in_ms)
        .map(|chunk| chunk.iter().sum())
        .collect();

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
