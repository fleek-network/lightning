# Lightning Metrics

**Lightning Metrics** is a crate designed to provide metrics for the Fleek Network's Lightning node implementation. This crate includes custom macros that can be used along with [autometrics-rs](https://github.com/autometrics-dev/autometrics-rs) used by the core

## Custom Macros

### `increment_counter!`

A macro to increment a counter metric.

#### Usage

```rust
increment_counter!("metric_name", Some("metric_description"), "label1" => "value1", "label2" => "value2");
```

### `histogram!`

A macro to record values in a histogram.

#### Usage

```rust
histogram!("histogram_name", Some("histogram_description"), value, bucket1, bucket2, ...);
```