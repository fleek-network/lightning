use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::transports;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct HandshakeConfig {
    /// List of transports to enable
    #[serde(rename = "transport")]
    pub transports: Vec<TransportConfig>,
    /// Shared tranport http address
    pub http_address: SocketAddr,
    /// Optional http configuration
    pub https: Option<HttpsConfig>,
    /// Timeout for disconnected sessions
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for HandshakeConfig {
    fn default() -> Self {
        Self {
            transports: vec![
                TransportConfig::WebRTC(Default::default()),
                TransportConfig::WebTransport(Default::default()),
                TransportConfig::Tcp(Default::default()),
                TransportConfig::Http(Default::default()),
            ],
            http_address: ([0, 0, 0, 0], 80).into(),
            https: None,
            timeout: Duration::from_secs(1),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum TransportConfig {
    Mock(transports::mock::MockTransportConfig),
    Tcp(transports::tcp::TcpConfig),
    WebRTC(transports::webrtc::WebRtcConfig),
    WebTransport(transports::webtransport::WebTransportConfig),
    Http(transports::http::Config),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HttpsConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub address: SocketAddr,
}
