use std::sync::Arc;
use std::time::SystemTime;

use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, CertificateError, Error, ServerName};

pub struct CertificateVerifier {
    pub(crate) hashes: Vec<Vec<u8>>,
}

impl ServerCertVerifier for CertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &Certificate,
        intermediates: &[Certificate],
        _: &ServerName,
        _: &mut dyn Iterator<Item = &[u8]>,
        _: &[u8],
        _: SystemTime,
    ) -> Result<ServerCertVerified, Error> {
        if !intermediates.is_empty() {
            // Todo: Circle back. It's not in the w3c spec.
            return Err(Error::General(
                "only one certificate required for validation".to_string(),
            ));
        }
        let hash_to_verify = ring::digest::digest(&ring::digest::SHA256, end_entity.as_ref());
        for hash in &self.hashes {
            if hash != hash_to_verify.as_ref() {
                return Err(Error::InvalidCertificate(CertificateError::Other(
                    Arc::from(Box::from("invalid hash given the certificate")),
                )));
            }
        }

        Ok(ServerCertVerified::assertion())
    }
}
