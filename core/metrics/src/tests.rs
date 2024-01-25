#[cfg(test)]
use autometrics::settings::AutometricsSettingsBuilder;
use lightning_types::{DEFAULT_HISTOGRAM_BUCKETS, METRICS_SERVICE_NAME};

use crate::{histogram, increment_counter};

fn init() {
    let _ = AutometricsSettingsBuilder::default()
        .service_name(METRICS_SERVICE_NAME)
        .histogram_buckets(DEFAULT_HISTOGRAM_BUCKETS)
        .try_init();
}

#[test]
fn test_counter_macro() {
    init();
    for _i in 0..5 {
        increment_counter!("Test_Custom_Counter", Some("A custom counter"), "extra_label1" => "1", "extra_label2" => "2");
    }
    increment_counter!("Test_Custom_Counter", Some("A custom counter"), "extra_label1" => "2", "extra_label2" => "4");
    increment_counter!("Test_Custom_Counter", Some("A custom counter"), "extra_label1" => "2", "extra_label2" => "5");

    let metric_families = prometheus::gather();
    let counter_metrics = metric_families
        .iter()
        .filter(|mf| mf.get_name() == "Test_Custom_Counter");

    for metric_family in counter_metrics {
        let metrics = metric_family.get_metric();
        for metric in metrics.iter() {
            let counter_value = metric.get_counter().get_value();
            assert!(counter_value >= 0.0, "Counter value is negative");
        }
    }
}

#[test]
fn test_histogram_macro() {
    init();
    histogram!(
        "Test_Custom_Histogram",
        Some("A custom histogram"),
        0.2,
        0.4,
        0.8,
        3.0,
        4.0,
        5.0,
        6.0
    );
    histogram!("Test_Custom_Histogram", Some("A custom histogram"), 0.3);
    histogram!("Test_Custom_Histogram", Some("A custom histogram"), 0.6);

    let metric_families = prometheus::gather();
    let histogram_metrics = metric_families
        .iter()
        .filter(|mf| mf.get_name() == "Test_Custom_Histogram");

    for metric_family in histogram_metrics {
        let metrics = metric_family.get_metric();
        for metric in metrics.iter() {
            let histogram = metric.get_histogram();

            let sum = histogram.get_sample_sum();
            assert!(sum >= 0.0, "Histogram sum is negative");
            assert_eq!(sum, 1.1, "sum do not match");

            let count = histogram.get_sample_count();
            assert_eq!(count, 3, "count do not match");

            let buckets = histogram.get_bucket();
            if let Some(first_bucket) = buckets.first() {
                let first_bucket_count = first_bucket.get_cumulative_count();
                assert_eq!(first_bucket_count, 2, "first bucket shold be 2");
            } else {
                panic!("No buckets found for histogram");
            }
        }
    }
}

#[test]
fn test_labeled_event_counter() {
    init();

    let family = "synchronizer_usage";
    let description = "Counting how often synchronizer has been used per epoch";
    let events_count = 7;
    let epoch = 10.to_string();

    for _ in 0..events_count {
        increment_counter!(family, Some(description), "epoch" => epoch.as_str());
    }

    let metrics = prometheus::gather()
        .into_iter()
        .filter(|mf| mf.get_name() == family)
        .collect::<Vec<_>>();
    // Only 1 Event Family
    assert_eq!(metrics.len(), 1);

    assert_eq!(metrics[0].get_name(), family);
    assert_eq!(metrics[0].get_help(), description);

    // Only 1 Counter for single Event labeled with `epoch`
    let events = metrics[0].get_metric();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].get_counter().get_value(), events_count as f64);
}
