use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct WebTransportConfig {
    pub address: SocketAddr,
    pub keep_alive: Option<Duration>,
    pub certificate: Option<SerializedCertificate>,
}

impl WebTransportConfig {
    pub fn set_certificate(&mut self, certificate: SerializedCertificate) {
        self.certificate = Some(certificate);
    }
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4240).into(),
            keep_alive: None,
            certificate: None,
        }
    }
}

/// Certificate in the binary DER format.
#[derive(Deserialize, Serialize)]
pub struct SerializedCertificate {
    pub certificate: Vec<u8>,
    pub key: Vec<u8>,
}

impl SerializedCertificate {
    pub fn from_certificate(cert: rcgen::Certificate) -> Result<Self> {
        Ok(Self {
            certificate: cert.serialize_der()?,
            key: cert.serialize_private_key_der(),
        })
    }
}
