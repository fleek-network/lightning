//! TLS configuration based on libp2p TLS specs.
//!
//! See <https://github.com/libp2p/specs/blob/master/tls/tls.md>.
//! Based on rust-libp2p/transports/tls.

mod certificate;
mod verifier;

use std::sync::Arc;

pub use certificate::parse_unverified;
use fleek_crypto::{NodePublicKey, NodeSecretKey};

const LIGHTNING_ALPN: &[u8] = b"fleek/lightning";

/// Create a TLS client configuration.
#[allow(unused)]
pub fn make_client_config(
    secret_key: &NodeSecretKey,
    remote_peer_id: Option<NodePublicKey>,
) -> Result<rustls::ClientConfig, certificate::GenError> {
    let (certificate, secret_key) = certificate::generate(secret_key)?;

    let mut crypto = rustls::ClientConfig::builder()
        .with_cipher_suites(verifier::CIPHERSUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(verifier::PROTOCOL_VERSIONS)
        .expect("Cipher suites and kx groups are configured; qed")
        .with_custom_certificate_verifier(Arc::new(
            verifier::CertificateVerifier::with_remote_peer_id(remote_peer_id),
        ))
        .with_client_auth_cert(vec![certificate], secret_key)
        .expect("Client cert key DER is valid; qed");
    crypto.alpn_protocols = vec![LIGHTNING_ALPN.to_vec()];

    Ok(crypto)
}

/// Create a TLS server configuration.
#[allow(unused)]
pub fn make_server_config(
    secret_key: &NodeSecretKey,
) -> Result<rustls::ServerConfig, certificate::GenError> {
    let (certificate, secret_key) = certificate::generate(secret_key)?;

    let mut crypto = rustls::ServerConfig::builder()
        .with_cipher_suites(verifier::CIPHERSUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(verifier::PROTOCOL_VERSIONS)
        .expect("Cipher suites and kx groups are configured; qed")
        .with_client_cert_verifier(Arc::new(verifier::CertificateVerifier::new()))
        .with_single_cert(vec![certificate], secret_key)
        .expect("Server cert key DER is valid; qed");
    crypto.alpn_protocols = vec![LIGHTNING_ALPN.to_vec()];
    Ok(crypto)
}
