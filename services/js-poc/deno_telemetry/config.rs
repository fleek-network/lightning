use std::collections::HashMap;
use std::env;

use deno_core::anyhow::{anyhow, bail};
use deno_tls::{load_certs, load_private_keys, TlsKey, TlsKeys};
pub use opentelemetry_otlp::Protocol;
pub use opentelemetry_sdk::metrics::Temporality;

/// Configuration for the telemetry exporter
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// http/protobuf (default), http/json
    pub protocol: Protocol,
    pub endpoint: Option<String>,
    pub headers: HashMap<String, String>,
    /// cumulative (default), delta, lowmemory
    pub temporality: Temporality,
    pub client_config: HyperClientConfig,
}

impl TelemetryConfig {
    // Parse configuration from environment variables
    pub fn from_env() -> deno_core::anyhow::Result<Self> {
        Ok(Self {
            protocol: env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
                .ok()
                .map(|v| match v.to_lowercase().as_str() {
                    "grpc" => Ok(Protocol::Grpc),
                    "http/json" => Ok(Protocol::HttpJson),
                    "http/protobuf" => Ok(Protocol::HttpBinary),
                    _ => Err(anyhow!("invalid otlp protocol")),
                })
                .unwrap_or(Ok(Protocol::HttpBinary))?,
            endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            headers: env::var("OTEL_EXPORTER_OTLP_HEADERS")
                .ok()
                .map(|v| {
                    let lines: Result<HashMap<String, String>, deno_core::anyhow::Error> = v
                        .split(";")
                        .map(|l| {
                            l.split_once('=')
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .ok_or(anyhow!("invalid header value"))
                        })
                        .collect();
                    lines
                })
                .unwrap_or(Ok(HashMap::new()))?,
            temporality: {
                match env::var("OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE")
                    .ok()
                    .map(|s| s.to_lowercase())
                    .as_deref()
                {
                    Some("cumulative") | None => Temporality::Cumulative,
                    Some("delta") => Temporality::Delta,
                    Some("lowmemory") => Temporality::LowMemory,
                    _ => bail!(""),
                }
            },
            client_config: HyperClientConfig::from_env()?,
        })
    }
}

/// HTTP client configuration
#[derive(Clone, Debug)]
pub struct HyperClientConfig {
    pub ca_certs: Vec<Vec<u8>>,
    pub keys: TlsKeys,
}

impl HyperClientConfig {
    /// Parse configuration from environment variables as per opentelemetry
    pub fn from_env() -> deno_core::anyhow::Result<Self> {
        let ca_certs = match std::env::var("OTEL_EXPORTER_OTLP_CERTIFICATE") {
            Ok(path) => vec![std::fs::read(path)?],
            _ => vec![],
        };

        let keys = match (
            std::env::var("OTEL_EXPORTER_OTLP_CLIENT_KEY"),
            std::env::var("OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE"),
        ) {
            (Ok(key_path), Ok(cert_path)) => {
                let key = std::fs::read(key_path)?;
                let cert = std::fs::read(cert_path)?;

                let certs = load_certs(&mut std::io::Cursor::new(cert))?;
                let key = load_private_keys(&key)?.into_iter().next().unwrap();

                TlsKeys::Static(TlsKey(certs, key))
            },
            _ => TlsKeys::Null,
        };

        Ok(Self { ca_certs, keys })
    }
}
