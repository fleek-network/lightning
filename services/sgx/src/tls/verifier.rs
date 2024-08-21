use std::fmt::Debug;
use std::sync::Arc;

use anyhow::anyhow;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{self, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, OtherError, SignatureScheme};
use x509_parser::der_parser::Oid;
use x509_parser::prelude::{FromDer, X509Certificate};

use crate::tls::attestation::Params;

#[derive(Debug)]
pub struct RemoteAttestationVerifier(());

impl RemoteAttestationVerifier {
    pub fn new() -> Self {
        Self(())
    }
}

impl ServerCertVerifier for RemoteAttestationVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        verify_with_remote_attestation(end_entity, intermediates)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        crypto::verify_tls12_signature(
            message,
            cert,
            &dss,
            &CryptoProvider::get_default()
                .expect("Provider has been installed")
                .signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        crypto::verify_tls13_signature(
            message,
            cert,
            &dss,
            &CryptoProvider::get_default()
                .expect("Provider has been installed")
                .signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ED25519,
        ]
    }
}

fn verify_with_remote_attestation(
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
) -> Result<(), Error> {
    if !intermediates.is_empty() {
        return Err(Error::General(
            "ra-tls requires exactly one certificate".into(),
        ));
    }

    let x509 = X509Certificate::from_der(end_entity.as_ref())
        .map(|(_rest_input, x509)| x509)
        .map_err(|_| Error::InvalidCertificate(CertificateError::BadEncoding))?;

    let quote_oid = Oid::from(&[0, 0, 0, 0])?;
    let tcb_info_oid = Oid::from(&[1, 0, 0, 0])?;
    let tcb_sign_chain_oid = Oid::from(&[2, 0, 0, 0])?;
    let pck_certificate_oid = Oid::from(&[3, 0, 0, 0])?;
    let pck_sign_chain_oid = Oid::from(&[4, 0, 0, 0])?;
    let cert_revocation_list_oid = Oid::from(&[5, 0, 0, 0])?;
    let quoting_enclave_id_oid = Oid::from(&[6, 0, 0, 0])?;

    let mut quote = None;
    let mut tcb_info = None;
    let mut tcb_sign_chain = None;
    let mut pck_certificate = None;
    let mut pck_sign_chain = None;
    let mut cert_revocation_list = None;
    let mut quoting_enclave_id = None;

    for ext in x509.extensions() {
        match &ext.oid {
            oid if &quote_oid == oid => {
                if quote.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                quote = Some(ext.value)
            },
            oid if &tcb_info_oid == oid => {
                if tcb_info.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                tcb_info = Some(ext.value)
            },
            oid if &tcb_sign_chain_oid == oid => {
                if tcb_sign_chain.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                tcb_sign_chain = Some(ext.value)
            },
            oid if &pck_certificate_oid == oid => {
                if pck_certificate.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                pck_certificate = Some(ext.value)
            },
            oid if &pck_sign_chain_oid == oid => {
                if pck_sign_chain.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                pck_sign_chain = Some(ext.value)
            },
            oid if &cert_revocation_list_oid == oid => {
                if cert_revocation_list.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                cert_revocation_list = Some(ext.value)
            },
            oid if &quoting_enclave_id_oid == oid => {
                if quoting_enclave_id.is_some() {
                    return Err(Error::General("duplicate extension".into()));
                }

                quoting_enclave_id = Some(ext.value)
            },
            _ => {
                return Err(Error::InvalidCertificate(
                    OtherError(Arc::new(anyhow!("unknown OID"))).into(),
                ));
            },
        }

        let _params = Params {
            quote: quote.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing quote extension"))).into(),
            ))?,
            tcb_info: tcb_info.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing tcb_info extension"))).into(),
            ))?,
            tcb_sign_chain: tcb_sign_chain.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing tcb_sign_chain extension"))).into(),
            ))?,
            pck_certificate: pck_certificate.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing pck_certificate extension"))).into(),
            ))?,
            pck_sign_chain: pck_sign_chain.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing pck_sign_chain extension"))).into(),
            ))?,
            cert_revocation_list: cert_revocation_list.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing cert_revocation_list extension"))).into(),
            ))?,
            quoting_enclave_id: quoting_enclave_id.ok_or(Error::InvalidCertificate(
                OtherError(Arc::new(anyhow!("missing quoting_enclave_id extension"))).into(),
            ))?,
        };

        // attestation(_params)?;
    }

    Ok(())
}
