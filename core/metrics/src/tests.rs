use crate::{histogram, increment_counter};

#[test]
fn test_counter_macro() {
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
