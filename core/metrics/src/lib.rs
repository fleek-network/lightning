pub mod counter;
pub mod histogram;
pub mod labels;
#[cfg(test)]
pub mod tests;

pub const METRICS_SERVICE_NAME: &str = "lightning_metrics";

pub const DEFAULT_HISTOGRAM_BUCKETS: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];
