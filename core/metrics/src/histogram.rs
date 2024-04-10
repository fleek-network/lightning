use dashmap::DashMap;
use once_cell::sync::Lazy;
use prometheus::core::Collector;
use prometheus::{register_histogram_vec, HistogramVec};
pub use stdext::function_name;
use tracing::error;

use crate::labels::Labels;
use crate::DEFAULT_HISTOGRAM_BUCKETS;

static HISTOGRAM: Lazy<DashMap<String, HistogramVec>> = Lazy::new(DashMap::new);

pub trait Histogram {
    fn observe(
        family: &str,
        description: Option<&str>,
        labels: &[&str],
        label_values: &[&str],
        value: f64,
        buckets: Option<Vec<f64>>,
    );
}

impl Histogram for Labels {
    fn observe(
        family: &str,
        description: Option<&str>,
        labels: &[&str],
        label_values: &[&str],
        value: f64,
        buckets: Option<Vec<f64>>,
    ) {
        let existing_buckets: Option<Vec<_>> = {
            HISTOGRAM.get(family).and_then(|existing_buckets| {
                let families: Vec<_> = existing_buckets.clone().collect();
                families
                    .first()
                    .and_then(|f| f.get_metric().first())
                    .map(|metric| metric.get_histogram().get_bucket().to_vec())
            })
        };

        if let Some(existing_buckets) = &existing_buckets {
            let bounds: Vec<f64> = existing_buckets
                .iter()
                .map(|b| b.get_upper_bound())
                .collect();
            if let Some(provided_buckets) = &buckets {
                if bounds != *provided_buckets {
                    error!(
                        "Mismatched buckets for family '{}'. Existing buckets: {:?}, New buckets: {:?}",
                        family, existing_buckets, buckets
                    );
                    return;
                }
            }
        };

        let histogram = HISTOGRAM.entry(family.to_string()).or_insert_with(|| {
            register_histogram_vec!(
                family,
                description.unwrap_or_default(),
                labels,
                buckets.unwrap_or(DEFAULT_HISTOGRAM_BUCKETS.to_vec())
            )
            .unwrap()
        });
        histogram.with_label_values(label_values).observe(value);
    }
}

#[macro_export]
macro_rules! histogram {
    ($family:expr, $description:expr, $value:expr, $($bucket:expr),+ ) => {
        {
            let buckets = vec![$($bucket),+];
            let function =
                $crate::labels::Labels::extract_fn_name($crate::histogram::function_name!());
            let default_labels = $crate::labels::Labels::new(function, module_path!());
            let default_labels = default_labels.to_vec();

            let all_labels: Vec<_> = default_labels
                .iter().map(|a| a.0).collect();
            let all_values: Vec<_> = default_labels
                .iter().map(|a| a.1).collect();
            <$crate::labels::Labels as $crate::histogram::Histogram>::observe(
                $family, $description, &all_labels, &all_values, $value, Some(buckets)
            );
        }
    };
    ($family:expr, $description:expr, $value:expr ) => {
    {
        let function =
            $crate::labels::Labels::extract_fn_name($crate::histogram::function_name!());
        let default_labels = $crate::labels::Labels::new(function, module_path!());
        let default_labels = default_labels.to_vec();

        let all_labels: Vec<_> = default_labels
            .iter().map(|a| a.0).collect();
        let all_values: Vec<_> = default_labels
            .iter().map(|a| a.1).collect();
        <$crate::labels::Labels as $crate::histogram::Histogram>::observe(
            $family, $description, &all_labels, &all_values, $value, None
        );
    }
    };
}
