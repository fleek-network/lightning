use std::{
    collections::BTreeMap,
    error::Error,
    time::{Duration, Instant},
};

use base64::Engine;
use csv::ReaderBuilder;
use lightning_topology::{clustering, divisive::DivisiveHierarchy};
use ndarray::{Array, Dim};
use ndarray_rand::rand_distr::{Distribution, UnitDisc};
use plotters::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

const FONT: &str = "IBM Plex Mono, monospace";

fn main() {
    toy_example();
    run();
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ServerData {
    id: u16,
    title: String,
    country: String,
    latitude: f32,
    longitude: f32,
}

#[derive(Clone, Debug)]
struct ClusterMetrics {
    mean_latency: f64,
    standard_dev_latency: f64,
    min_latency: f64,
    max_latency: f64,
    count: usize,
}

impl From<Vec<f64>> for ClusterMetrics {
    fn from(value: Vec<f64>) -> Self {
        let mean_latency = mean(&value).unwrap_or(f64::NAN);
        let standard_dev_latency = std_deviation(&value).unwrap_or(f64::NAN);
        let mut min_latency = f64::MAX;
        let mut max_latency = f64::MIN;
        value.iter().for_each(|v| {
            min_latency = min_latency.min(*v);
            max_latency = max_latency.max(*v);
        });

        if min_latency == f64::MAX {
            min_latency = f64::NAN
        }

        if max_latency == f64::MIN {
            max_latency = f64::NAN
        }

        Self {
            mean_latency,
            standard_dev_latency,
            min_latency,
            max_latency,
            count: ((value.len() + 1) as f64).sqrt().round() as usize,
        }
    }
}

fn get_table_rows(cluster_metrics: &BTreeMap<usize, ClusterMetrics>) -> String {
    let mut table_rows = vec![];
    for (cluster_idx, metrics) in cluster_metrics.iter() {
        table_rows.push(format!(
            "<tr><td>{cluster_idx}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            metrics.count,
            metrics.mean_latency,
            metrics.standard_dev_latency,
            metrics.min_latency,
            metrics.max_latency
        ));
    }
    table_rows.join("\n")
}

fn histogram(values: &[f64], min_val: f64, max_val: f64, title: &str) -> String {
    let root = BitMapBackend::new("/tmp/histogram.png", (600, 600)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let data: Vec<u32> = values.iter().map(|v| v.round() as u32).collect();

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(40)
        .margin(5)
        .caption(title, ("monospace", 20.0))
        .build_cartesian_2d(
            (min_val as u32..max_val as u32).into_segmented(),
            0u32..10u32,
        )
        .unwrap();

    chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .y_desc("Count")
        .x_desc("Bucket")
        .axis_desc_style(("monospace", 15))
        .draw()
        .unwrap();

    chart
        .draw_series(
            Histogram::vertical(&chart)
                .style(BLUE.mix(0.5).filled())
                .data(data.iter().map(|x: &u32| (*x, 1))),
        )
        .unwrap();

    root.present()
        .expect("Unable to write temp histogram to file");

    let file = std::fs::read("/tmp/histogram.png").expect("failed to read temp file");
    base64::engine::general_purpose::STANDARD.encode(file)
}

fn scatter_plot(
    buffer: &mut String,
    data: &Array<f64, Dim<[usize; 2]>>,
    assignment: &[usize],
    title: &str,
) {
    let mut series = BTreeMap::new();
    let mut x_min = f64::MAX;
    let mut x_max = f64::MIN;
    let mut y_min = f64::MAX;
    let mut y_max = f64::MIN;
    for (i, cluster_index) in assignment.iter().enumerate() {
        let x = data[[i, 0]];
        let y = data[[i, 1]];

        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_min = y_min.min(y);
        y_max = y_max.max(y);
        series
            .entry(cluster_index)
            .or_insert(Vec::new())
            .push((x, y));
    }

    let root_area = SVGBackend::with_string(buffer, (1200, 800)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    // make the plot wider
    x_min -= 5.;
    x_max += 5.;
    y_min -= 40.;
    y_max += 20.;

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption(title, (FONT, 40))
        .build_cartesian_2d(x_min..x_max, y_min..y_max)
        .unwrap();

    ctx.configure_mesh().draw().unwrap();

    let color_map = DerivedColorMap::new(&[
        RGBColor(230, 25, 75),
        RGBColor(60, 180, 75),
        RGBColor(255, 225, 25),
        RGBColor(0, 130, 200),
        RGBColor(245, 130, 48),
        RGBColor(145, 30, 180),
        RGBColor(70, 240, 240),
        RGBColor(240, 50, 230),
        RGBColor(210, 245, 60),
        RGBColor(250, 190, 212),
        RGBColor(0, 128, 128),
        RGBColor(220, 190, 255),
        RGBColor(170, 110, 40),
        RGBColor(255, 250, 200),
        RGBColor(128, 0, 0),
        RGBColor(170, 255, 195),
        RGBColor(128, 128, 0),
        RGBColor(255, 215, 180),
        RGBColor(0, 0, 128),
        RGBColor(128, 128, 128),
        RGBColor(0, 0, 0),
    ]);

    series.iter().for_each(|(&cluster_index, points)| {
        let color = color_map.get_color(*cluster_index as f64 / series.len() as f64);

        ctx.draw_series(points.iter().map(|point| Circle::new(*point, 5, color)))
            .unwrap()
            .label(format!("Cluster {cluster_index} ({})", points.len()))
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color));
    });

    ctx.configure_series_labels()
        .border_style(BLACK)
        .background_style(WHITE.mix(0.8))
        .label_font(FONT)
        .margin(15)
        .position(SeriesLabelPosition::LowerLeft)
        .draw()
        .unwrap();
}

fn run_constrained_fasterpam(
    dis_matrix: &Array<i32, Dim<[usize; 2]>>,
    num_clusters: usize,
    min: usize,
    max: usize,
) -> (Vec<usize>, usize, Duration) {
    let mut meds =
        rand::seq::index::sample(&mut rand::thread_rng(), dis_matrix.nrows(), num_clusters)
            .into_vec();
    let instant = Instant::now();
    let (_, labels, iterations, _): (f64, _, _, _) =
        clustering::constrained_fasterpam(dis_matrix, &mut meds, 100, min, max);
    (labels, iterations, instant.elapsed())
}

fn run_divisive_constrained_fasterpam(
    dis_matrix: &Array<i32, Dim<[usize; 2]>>,
    target_n: usize,
) -> (Vec<Vec<usize>>, Duration) {
    let instant = Instant::now();
    let mut rng = rand::thread_rng();
    let hierarchy = DivisiveHierarchy::new(&mut rng, dis_matrix, target_n);
    let json = to_string_pretty(&hierarchy).expect("failed to serialize divisive topology");
    std::fs::write("divisive_toplogy.json", json).expect("failed to save divisive toplogoy");
    let labels = hierarchy.assignments();
    (labels, instant.elapsed())
}

fn get_random_assignment(num_clusters: usize, num_nodes: usize) -> Vec<usize> {
    let mut rng = rand::thread_rng();

    let mut assignments = vec![0; num_nodes];
    for assignment in assignments.iter_mut() {
        let cluster_index = rng.gen_range(0..num_clusters);
        *assignment = cluster_index;
    }
    assignments
}

fn read_latency_matrix(path: &str) -> Result<Vec<Vec<f32>>, Box<dyn Error>> {
    let mut buf = Vec::new();

    let mut rdr = ReaderBuilder::new().has_headers(false).from_path(path)?;
    for result in rdr.deserialize() {
        let record: Vec<f32> = result?;
        buf.push(record);
    }

    Ok(buf)
}

fn read_metadata(path: &str) -> Result<BTreeMap<u16, ServerData>, Box<dyn Error>> {
    let mut metadata = BTreeMap::new();
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    for result in rdr.deserialize() {
        let record: ServerData = result?;
        metadata.insert(record.id, record);
    }
    Ok(metadata)
}

pub fn mean(data: &[f64]) -> Option<f64> {
    let sum = data.iter().sum::<f64>();
    let count = data.len();

    (count > 0).then_some(sum / count as f64)
}

pub fn std_deviation(data: &[f64]) -> Option<f64> {
    match (mean(data), data.len()) {
        (Some(data_mean), count) if count > 0 => {
            let variance = data
                .iter()
                .map(|value| {
                    let diff = data_mean - (*value);

                    diff * diff
                })
                .sum::<f64>()
                / count as f64;

            Some(variance.sqrt())
        },
        _ => None,
    }
}

fn calculate_cluster_metrics(
    assignment: &[usize],
    latency_matrix: &[Vec<f32>],
) -> (BTreeMap<usize, ClusterMetrics>, ClusterMetrics) {
    let mut clusters = BTreeMap::new();
    for (i, cluster_index) in assignment.iter().enumerate() {
        clusters.entry(cluster_index).or_insert(Vec::new()).push(i);
    }
    let mut latency_values = Vec::new();

    let mut cluster_metrics_map = BTreeMap::new();
    for (&cluster_idx, node_indices) in clusters.iter() {
        if node_indices.is_empty() {
            continue;
        }
        let mut cluster_latency_values = Vec::new();
        for &src in node_indices.iter() {
            for &dst in node_indices.iter() {
                if src != dst {
                    let latency = latency_matrix[src][dst] as f64;
                    cluster_latency_values.push(latency);
                }
            }
        }
        let cluster_metrics: ClusterMetrics = cluster_latency_values.into();
        cluster_metrics_map.insert(*cluster_idx, cluster_metrics.clone());

        latency_values.push(cluster_metrics.mean_latency);
    }
    let cluster_metrics: ClusterMetrics = latency_values.into();

    (cluster_metrics_map, cluster_metrics)
}

fn sample_cluster(
    x: f64,
    y: f64,
    x_scale: f64,
    y_scale: f64,
    num_points: usize,
) -> Array<f64, Dim<[usize; 2]>> {
    let mut cluster = Array::zeros((num_points, 2));

    for i in 0..num_points {
        let v: [f64; 2] = UnitDisc.sample(&mut rand::thread_rng());
        cluster[[i, 0]] = x + v[0] * x_scale;
        cluster[[i, 1]] = y + v[1] * y_scale;
    }
    cluster
}

fn get_distance_matrix(data: &Array<f64, Dim<[usize; 2]>>) -> Array<f64, Dim<[usize; 2]>> {
    let mut dist = Array::zeros((data.shape()[0], data.shape()[0]));
    for i in 0..data.shape()[0] {
        for j in 0..data.shape()[0] {
            if i != j {
                dist[[i, j]] =
                    (data[[i, 0]] - data[[j, 0]]).powi(2) + (data[[i, 1]] - data[[j, 1]]).powi(2);
            }
        }
    }
    dist
}

fn toy_example() {
    let cluster1 = sample_cluster(1., 1., 1., 1., 10);
    let cluster2 = sample_cluster(4., 3., 1., 1., 10);
    let cluster3 = sample_cluster(5., 5., 1., 1., 20);
    let data = ndarray::concatenate(
        ndarray::Axis(0),
        &[(&cluster1).into(), (&cluster2).into(), (&cluster3).into()],
    )
    .unwrap();

    let mut plot_buffer = String::new();
    scatter_plot(
        &mut plot_buffer,
        &data,
        &vec![0; data.shape()[0]],
        "Before Clustering",
    );

    let dis_matrix = get_distance_matrix(&data);
    let mut dis_matrix_i32 = Array::zeros((dis_matrix.shape()[0], dis_matrix.shape()[1]));
    for i in 0..dis_matrix.shape()[0] {
        for j in 0..dis_matrix.shape()[1] {
            dis_matrix_i32[[i, j]] = (dis_matrix[[i, j]] * 1000.0) as i32;
        }
    }

    let (assignment, _, _) = run_constrained_fasterpam(&dis_matrix_i32, 3, 4, 12);
    scatter_plot(
        &mut plot_buffer,
        &data,
        &assignment,
        "WIP Constrained FasterPAM",
    );

    let html = format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Topology Simulation Report</title>
    <style>
        @import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono&display=swap');
        body {{ font-family: '{FONT}', monospace }}
        tr:nth-child(even) {{
            background-color: rgba(150, 212, 212, 0.4);
        }}
    </style>
</head>
<body>
    <h1>Toy Example Report</h1>
    <p>The clustering </p>
    <hr>
    {plot_buffer}
    <hr>
<body>
</html>
          "#
    );
    std::fs::write("toy_report.html", html).unwrap();
}

fn run() {
    let matrix = read_latency_matrix("data/matrix.csv").unwrap();
    let metadata = read_metadata("data/metadata.csv").unwrap();

    let mut data_points = Array::zeros((metadata.len(), 2));
    metadata
        .iter()
        .enumerate()
        .for_each(|(i, (_, server_data))| {
            data_points[[i, 1]] = server_data.latitude as f64;
            data_points[[i, 0]] = server_data.longitude as f64;
        });

    let mut plot_buffer = String::new();

    scatter_plot(
        &mut plot_buffer,
        &data_points,
        &vec![0; data_points.shape()[0]],
        "Before Clustering",
    );

    let mut dissim_matrix = Array::zeros((matrix.len(), matrix.len()));
    for i in 0..matrix.len() {
        for j in 0..matrix.len() {
            dissim_matrix[[i, j]] = (matrix[i][j] * 1000.0) as i32;
        }
    }

    let num_servers = dissim_matrix.shape()[0];
    let optimal_cluster_size = 10;
    let num_clusters = num_servers / optimal_cluster_size;

    /* BASELINE: RANDOM ASSIGNMENT */

    // establish baseline by random assignment
    let random_assignment = get_random_assignment(num_clusters, num_servers);
    scatter_plot(
        &mut plot_buffer,
        &data_points,
        &random_assignment,
        "Random Assignment",
    );
    let (metrics_for_each_cluster_baseline, overall_cluster_metrics_baseline) =
        calculate_cluster_metrics(&random_assignment, &matrix);
    let table_rows_baseline = get_table_rows(&metrics_for_each_cluster_baseline);

    /* CONSTRAINED FASTERPAM */

    eprintln!("running constrained fasterpam");

    let (assignment, c_fasterpam_num_iterations, c_fasterpam_duration) =
        run_constrained_fasterpam(&dissim_matrix, num_clusters, 8, 10);
    scatter_plot(
        &mut plot_buffer,
        &data_points,
        &assignment,
        "Constrained FasterPAM",
    );

    let (metrics_for_each_cluster_c_fasterpam, overall_cluster_metrics_c_fasterpam) =
        calculate_cluster_metrics(&assignment, &matrix);
    let table_rows_c_fasterpam = get_table_rows(&metrics_for_each_cluster_c_fasterpam);

    /* DIVISIVE CONSTRAINED FASTERPAM */

    eprintln!("running divisive constrained fasterpam");

    let (hierarchy_assignments, dcfpam_duration) =
        run_divisive_constrained_fasterpam(&dissim_matrix, 8);
    plot_buffer.push_str(r#"<div class="side-by-side" style="display: flex;">"#);
    for (depth, assignment) in hierarchy_assignments.iter().skip(1).enumerate() {
        scatter_plot(
            &mut plot_buffer,
            &data_points,
            assignment,
            &format!("Divisive Constrained Fastpam - Level {depth}"),
        );
    }
    plot_buffer.push_str(r#"</div>"#);

    let (metrics_for_each_cluster_dcfpam, overall_cluster_metrics_dcfpam) =
        calculate_cluster_metrics(hierarchy_assignments.last().unwrap(), &matrix);
    let table_rows_dcfpam = get_table_rows(&metrics_for_each_cluster_dcfpam);

    /* BOTTOM UP CONSTRAINED FASTERPAM */

    // build histograms
    let baseline_cluster_means = metrics_for_each_cluster_baseline
        .values()
        .map(|metrics| metrics.mean_latency)
        .collect::<Vec<f64>>();
    let random_assignment_latency_histogram = histogram(
        &baseline_cluster_means,
        overall_cluster_metrics_baseline.min_latency,
        overall_cluster_metrics_baseline.max_latency,
        "Random Assignment Cluster Latency",
    );
    let baseline_cluster_sizes: Vec<f64> = metrics_for_each_cluster_baseline
        .values()
        .map(|metrics| metrics.count as f64)
        .collect();
    let random_assignment_sizes_histogram = histogram(
        &baseline_cluster_sizes,
        baseline_cluster_sizes
            .iter()
            .fold(f64::MAX, |a, &b| a.min(b)),
        baseline_cluster_sizes
            .iter()
            .fold(f64::MIN, |a, &b| a.max(b)),
        "Random Assignment Cluster Sizes",
    );

    let c_fasterpam_cluster_means = metrics_for_each_cluster_c_fasterpam
        .values()
        .map(|metrics| metrics.mean_latency)
        .collect::<Vec<f64>>();
    let c_fasterpam_latency_histogram = histogram(
        &c_fasterpam_cluster_means,
        overall_cluster_metrics_c_fasterpam.min_latency,
        overall_cluster_metrics_c_fasterpam.max_latency,
        "Constrained FasterPAM Cluster Latency",
    );

    let c_fasterpam_cluster_sizes: Vec<f64> = metrics_for_each_cluster_c_fasterpam
        .values()
        .map(|metrics| metrics.count as f64)
        .collect();

    let c_fasterpam_sizes_histogram = histogram(
        &c_fasterpam_cluster_sizes,
        c_fasterpam_cluster_sizes
            .iter()
            .fold(f64::MAX, |a, &b| a.min(b)),
        c_fasterpam_cluster_sizes
            .iter()
            .fold(f64::MIN, |a, &b| a.max(b)),
        "Constrained FasterPAM Cluster Sizes",
    );

    let dcfpam_cluster_sizes: Vec<f64> = metrics_for_each_cluster_dcfpam
        .values()
        .map(|metrics| metrics.count as f64)
        .collect();
    let dcfpam_sizes_histogram = histogram(
        &dcfpam_cluster_sizes,
        dcfpam_cluster_sizes.iter().fold(f64::MAX, |a, &b| a.min(b)),
        dcfpam_cluster_sizes.iter().fold(f64::MIN, |a, &b| a.max(b)),
        "Divisive Constrained FasterPAM Cluster Sizes",
    );
    let dcfpam_cluster_means = metrics_for_each_cluster_dcfpam
        .values()
        .map(|metrics| metrics.mean_latency)
        .collect::<Vec<f64>>();
    let dcfpam_latency_histogram = histogram(
        &dcfpam_cluster_means,
        overall_cluster_metrics_dcfpam.min_latency,
        overall_cluster_metrics_dcfpam.max_latency,
        "Divisive Constrained FasterPAM Cluster Latency",
    );

    // create html report
    let html = format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Topology Simulation Report</title>
    <style>
        @import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono&display=swap');
        body {{ font-family: '{FONT}', monospace }}
        tr:nth-child(even) {{
            background-color: rgba(150, 212, 212, 0.4);
        }}

        .side-by-side > * {{
            min-width: 1200px;
        }}
    </style>
</head>
<body>
    <h1>Topology Simulation Report</h1>
    <p>The clustering </p>
    <hr>
    {plot_buffer}
    <hr>
    <div style="display: flex;">
        <div style="width:100%; margin:1%;">
            <h2>Random Assignment</h2>
            <p>
                Avg. Latency: {}</br>
                Std. Dev. Latency: {}</br>
                Min. Latency: {}</br>
                Max. Latency: {}</br>
            </p>
            <table>
                <tr>
                    <th>Cluster</th>
                    <th>Count</th>
                    <th>Avg. Latency</th>
                    <th>Std. Dev. Latency</th>
                    <th>Min. Latency</th>
                    <th>Max. Latency</th>
                </tr>
                {}
            </table>
        </div>
        <div style="width:100%; margin:1%;">
            <h2>Constrained FasterPAM</h2>
            <p>
                Duration: {c_fasterpam_duration:?}</br>
                Num. Iterations: {}</br>
                Avg. Latency: {}</br>
                Std. Dev. Latency: {}</br>
                Min. Latency: {}</br>
                Max. Latency: {}</br>
            </p>
            <table>
                <tr>
                    <th>Cluster</th>
                    <th>Count</th>
                    <th>Avg. Latency</th>
                    <th>Std. Dev. Latency</th>
                    <th>Min. Latency</th>
                    <th>Max. Latency</th>
                </tr>
                {}
            </table>
        </div>
        <div style="width:100%; margin:1%;">
            <h2>Divisive Constrained FasterPAM</h2>
            <p>
                Duration: {dcfpam_duration:?}</br>
                Avg. Latency: {}</br>
                Std. Dev. Latency: {}</br>
                Min. Latency: {}</br>
                Max. Latency: {}</br>
            </p>
            <table>
                <tr>
                    <th>Cluster</th>
                    <th>Count</th>
                    <th>Avg. Latency</th>
                    <th>Std. Dev. Latency</th>
                    <th>Min. Latency</th>
                    <th>Max. Latency</th>
                </tr>
                {}
            </table>
        </div>
    </div>

    <h1>Cluster Latency Histograms</h1>
    <div style="display: flex;">
        <img src="data:image/png;base64,{random_assignment_latency_histogram}" width="500" />
        <img src="data:image/png;base64,{c_fasterpam_latency_histogram}" width="500" />
        <img src="data:image/png;base64,{dcfpam_latency_histogram}" width="500" />
    </div>

    <h1>Cluster Sizes Histograms</h1>
    <div style="display: flex;">
        <img src="data:image/png;base64,{random_assignment_sizes_histogram}" width="500" />
        <img src="data:image/png;base64,{c_fasterpam_sizes_histogram}" width="500" />
        <img src="data:image/png;base64,{dcfpam_sizes_histogram}" width="500" />
    </div>
<body>
</html>
          "#,
        overall_cluster_metrics_baseline.mean_latency,
        overall_cluster_metrics_baseline.standard_dev_latency,
        overall_cluster_metrics_baseline.min_latency,
        overall_cluster_metrics_baseline.max_latency,
        table_rows_baseline,
        c_fasterpam_num_iterations,
        overall_cluster_metrics_c_fasterpam.mean_latency,
        overall_cluster_metrics_c_fasterpam.standard_dev_latency,
        overall_cluster_metrics_c_fasterpam.min_latency,
        overall_cluster_metrics_c_fasterpam.max_latency,
        table_rows_c_fasterpam,
        overall_cluster_metrics_dcfpam.mean_latency,
        overall_cluster_metrics_dcfpam.standard_dev_latency,
        overall_cluster_metrics_dcfpam.min_latency,
        overall_cluster_metrics_dcfpam.max_latency,
        table_rows_dcfpam,
    );

    std::fs::write("report.html", html).unwrap();
}
