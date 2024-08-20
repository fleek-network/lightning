use anyhow::Result;
use rcgen::CustomExtension;

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
