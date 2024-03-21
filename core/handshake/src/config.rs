use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::transports;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct HandshakeConfig {
    #[serde(rename = "transport")]
    pub transports: Vec<TransportConfig>,
    pub http_address: SocketAddr,
    pub https: Option<HttpsConfig>,
    pub use_ebpf_service: bool,
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
            http_address: ([0, 0, 0, 0], 4220).into(),
            https: None,
            use_ebpf_service: false,
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
