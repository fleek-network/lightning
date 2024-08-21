use anyhow::Result;
use rcgen::CustomExtension;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

pub fn generate(_: Vec<u8>) -> Result<(CertificateDer, PrivateKeyDer)> {
    let certificate_keypair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let rustls_key: PrivateKeyDer = certificate_keypair.serialize_der().try_into()?;

    let certificate = {
        let mut params = rcgen::CertificateParams::new(vec![])?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.custom_extensions =
            extensions(vec![], vec![], vec![], vec![], vec![], vec![], vec![])?;
        params.self_signed(&certificate_keypair)?
    };

    let rustls_certificate = CertificateDer(certificate.serialize_der()?);

    Ok((rustls_certificate, rustls_key))
}

// Todo: Choose appropriate OIDs.
pub fn extensions(
    quote: Vec<u8>,
    tcb_info: Vec<u8>,
    tcb_sign_chain: Vec<u8>,
    pck_certificate: Vec<u8>,
    pck_sign_chain: Vec<u8>,
    cert_revocation_list: Vec<u8>,
    quoting_enclave_id: Vec<u8>,
) -> Result<Vec<CustomExtension>> {
    let mut extensions = Vec::new();

    // Quote.
    extensions.push(CustomExtension::from_oid_content(&[0, 0, 0, 0], quote));

    // TCB Info.
    extensions.push(CustomExtension::from_oid_content(&[1, 0, 0, 0], tcb_info));

    // TCB Signing Chain.
    extensions.push(CustomExtension::from_oid_content(
        &[2, 0, 0, 0],
        tcb_sign_chain,
    ));

    // PCK Certificate.
    extensions.push(CustomExtension::from_oid_content(
        &[3, 0, 0, 0],
        pck_certificate,
    ));

    // PCK Signing Chain.
    extensions.push(CustomExtension::from_oid_content(
        &[4, 0, 0, 0],
        pck_sign_chain,
    ));

    // Certificate Revocation Lists.
    extensions.push(CustomExtension::from_oid_content(
        &[5, 0, 0, 0],
        cert_revocation_list,
    ));

    // Quoting Enclave Identity.
    extensions.push(CustomExtension::from_oid_content(
        &[6, 0, 0, 0],
        quoting_enclave_id,
    ));

    Ok(extensions)
}
