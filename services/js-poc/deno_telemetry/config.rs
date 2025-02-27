//! Configuration provided by the runtime to initialize the extension

use std::collections::HashMap;

pub use opentelemetry_otlp::Protocol;
pub use opentelemetry_sdk::metrics::Temporality;

#[derive(Debug)]
pub struct TelemetryConfig {
    /// http/protobuf (default), http/json
    pub protocol: Protocol,
    pub endpoint: Option<String>,
    pub headers: HashMap<String, String>,

    /// cumulative (default), delta, lowmemory
    pub temporality: Temporality,

    pub client_config: HyperClientConfig,
}

#[derive(Debug)]
pub struct HyperClientConfig {
    // hyper client config
    // pub cert: Option<String>,
    // pub client: Option<(String, String)>,
}

impl HyperClientConfig {
    // pub fn from_env() -> Self {
    //     Self {
    //         cert: env::var("OTEL_EXPORTER_OTLP_CERTIFICATE").ok(),
    //         client: env::var("OTEL_EXPORTER_OTLP_CLIENT_KEY")
    //             .ok()
    //             .and_then(|cert| {
    //                 env::var("OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE")
    //                     .ok()
    //                     .map(|key| (cert, key))
    //             }),
    //     }
    // }
}

impl TelemetryConfig {
    // Parse configuration from environment variables
    // pub fn from_env() -> deno_core::anyhow::Result<Self> {
    //     Ok(Self {
    //         protocol: env::var("OTEL_EXPORTER_OTLP_PROTOCOL").ok(),
    //         endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
    //         headers: env::var("OTEL_EXPORTER_OTLP_HEADERS").ok(),
    //         temporality: {
    //             match env::var("OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE")
    //                 .ok()
    //                 .map(|s| s.to_lowercase())
    //                 .as_deref()
    //             {
    //                 Some("cumulative") | None => Temporality::Cumulative,
    //                 Some("delta") => Temporality::Delta,
    //                 Some("lowmemory") => Temporality::LowMemory,
    //                 _ => bail!(""),
    //             }
    //         },
    //         client_config: HyperClientConfig::from_env(),
    //     })
    // }
}
