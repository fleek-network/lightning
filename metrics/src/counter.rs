use dashmap::DashMap;
use log::error;
use once_cell::sync::Lazy;
use prometheus::{core::Collector, register_int_counter_vec, IntCounterVec};

use crate::labels::Labels;

static COUNTERS: Lazy<DashMap<String, IntCounterVec>> = Lazy::new(DashMap::new);

pub trait Counter {
    fn increment(family: &str, description: Option<&str>, labels: &[&str], label_values: &[&str]);
}

impl Counter for Labels {
    fn increment(family: &str, description: Option<&str>, labels: &[&str], label_values: &[&str]) {
        let existing_labels: Option<Vec<_>> = {
            if let Some(existing_counter) = COUNTERS.get(family) {
                let families = existing_counter.clone().collect();
                families
                    .first()
                    .and_then(|f| f.get_metric().first())
                    .map(|metric| {
                        metric
                            .get_label()
                            .iter()
                            .map(|l| l.get_name().to_owned())
                            .collect()
                    })
            } else {
                None
            }
        };
        if let Some(existing_labels) = &existing_labels {
            let mut sorted_existing_labels = existing_labels.clone();
            let mut sorted_new_labels: Vec<_> = labels.to_vec();
            sorted_existing_labels.sort();
            sorted_new_labels.sort();

            if sorted_existing_labels != sorted_new_labels {
                error!(
                    "Mismatched labels for family '{}'. Existing labels: {:?}, New labels: {:?}",
                    family, existing_labels, labels
                );
                return;
            }
        };
        let counter = COUNTERS.entry(family.to_string()).or_insert_with(|| {
            register_int_counter_vec!(family, description.unwrap_or_default(), labels).unwrap()
        });

        counter.with_label_values(label_values).inc();
    }
}

#[macro_export]
macro_rules! increment_counter {
    ($family:expr, $description:expr, $($label:expr => $value:expr),*) => {
        {
            let function = Labels::extract_fn_name(function_name!());
            let default_labels = Labels::new(function, module_path!());
            let default_labels = default_labels.to_vec();

            let additional_labels = vec![$($label),*];
            let additional_values = vec![$($value),*];

            let all_labels: Vec<_> = default_labels
                .iter().map(|a| a.0).chain(additional_labels).collect();
            let all_values: Vec<_> = default_labels
                .iter().map(|a| a.1).chain(additional_values).collect();

            Labels::increment($family, $description, &all_labels, &all_values);
        }
    };
}
