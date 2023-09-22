mod verifier;

use std::sync::Arc;

use rustls::cipher_suite::{
    TLS13_AES_128_GCM_SHA256,
    TLS13_AES_256_GCM_SHA384,
    TLS13_CHACHA20_POLY1305_SHA256,
};
use rustls::{ClientConfig, SupportedCipherSuite, SupportedProtocolVersion};

use crate::tls::verifier::CertificateVerifier;

static PROTOCOL_VERSIONS: &[&SupportedProtocolVersion] = &[&rustls::version::TLS13];
static CIPHERSUITES: &[SupportedCipherSuite] = &[
    TLS13_CHACHA20_POLY1305_SHA256,
    TLS13_AES_256_GCM_SHA384,
    TLS13_AES_128_GCM_SHA256,
];
const DEFAULT_ALPN: &[u8] = b"h3";

pub fn tls_config(hashes: Vec<Vec<u8>>) -> ClientConfig {
    let mut crypto = rustls::ClientConfig::builder()
        .with_cipher_suites(CIPHERSUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(PROTOCOL_VERSIONS)
        .expect("Cipher suites and kx groups are configured")
        .with_custom_certificate_verifier(Arc::new(CertificateVerifier { hashes }))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![DEFAULT_ALPN.to_vec()];
    crypto
}
