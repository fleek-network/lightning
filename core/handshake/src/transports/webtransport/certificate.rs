use anyhow::Result;
use fleek_crypto::NodeSecretKey;
use rcgen::{CertificateParams, DistinguishedName, DnType, PKCS_ECDSA_P256_SHA256};
use time::{Duration, OffsetDateTime};

const COMMON_NAME: &str = "lightning";

pub fn generate_certificate(_sk: NodeSecretKey) -> Result<rcgen::Certificate> {
    let mut dname = DistinguishedName::new();
    dname.push(DnType::CommonName, COMMON_NAME);

    let mut params = CertificateParams::new(vec![
        // "localhost".to_string(),
        // "127.0.0.1".to_string(),
        COMMON_NAME.to_string(),
    ]);

    // params.key_pair = Some();
    // let key_pair = KeyPair::from_pem(&sk.encode_pem())?;

    params.alg = &PKCS_ECDSA_P256_SHA256;
    params.distinguished_name = dname;
    params.not_before = OffsetDateTime::now_utc();
    // Unwrap is OK because this gets called during start up and
    // it's unlikely to be an issue within our lifetime.
    params.not_after = OffsetDateTime::now_utc()
        .checked_add(Duration::days(14))
        .unwrap();
    rcgen::Certificate::from_params(params).map_err(Into::into)
}
