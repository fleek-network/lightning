use anyhow::Result;
use fleek_crypto::{NodeSecretKey, SecretKey};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ED25519};
use time::{Duration, OffsetDateTime};

const COMMON_NAME: &str = "localhost";

pub fn generate_certificate(sk: NodeSecretKey) -> Result<rcgen::Certificate> {
    let mut dname = DistinguishedName::new();
    dname.push(DnType::CommonName, COMMON_NAME);

    let key_pair = KeyPair::from_pem(&sk.encode_pem())?;

    let mut params = CertificateParams::new(vec![COMMON_NAME.to_string()]);
    params.key_pair = Some(key_pair);
    params.alg = &PKCS_ED25519;
    params.distinguished_name = dname;
    params.not_before = OffsetDateTime::now_utc();
    // Unwrap is OK because this gets called during start up and
    // it's unlikely to be an issue within our lifetime.
    params.not_after = OffsetDateTime::now_utc()
        .checked_add(Duration::days(14))
        .unwrap();
    rcgen::Certificate::from_params(params).map_err(Into::into)
}
