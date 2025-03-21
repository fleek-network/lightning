use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Configuration added to `fleek.config.json`
#[derive(Default, Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleekConfig {
    #[serde(default)]
    pub otel: OtelConfig,
}

/// Configuration related to the runtime's opentelemetry exporter
#[derive(Default, Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct OtelConfig {
    /// Optional endpoint to send otlp http logs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<deno_core::url::Url>,
    /// Optional headers to include when exporting otlp
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    /// Additional global tags to include with exported data
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub tags: HashMap<String, String>,
}
