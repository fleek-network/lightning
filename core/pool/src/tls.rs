use std::sync::Arc;

use rustls::{ClientConfig, ServerConfig};

pub static CIPHER_SUITES: &[rustls::SupportedCipherSuite] = &[
    rustls::cipher_suite::TLS13_AES_128_GCM_SHA256,
    rustls::cipher_suite::TLS13_AES_256_GCM_SHA384,
    rustls::cipher_suite::TLS13_CHACHA20_POLY1305_SHA256,
];

pub static PROTOCOL_VERSIONS: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

pub fn client_config() -> ClientConfig {
    let mut config = ClientConfig::builder()
        .with_cipher_suites(CIPHER_SUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(PROTOCOL_VERSIONS)
        .expect("Cipher suites and kx groups are configured; qed")
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();
    config.alpn_protocols = vec![b"gemini".to_vec()];
    config
}

pub fn server_config() -> ServerConfig {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let key = rustls::PrivateKey(cert.serialize_private_key_der());
    let cert = vec![rustls::Certificate(cert.serialize_der().unwrap())];

    let mut config = ServerConfig::builder()
        .with_cipher_suites(CIPHER_SUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(PROTOCOL_VERSIONS)
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .expect("Building server config to suceed");
    config.alpn_protocols = vec![b"gemini".to_vec()];
    config
}

struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
