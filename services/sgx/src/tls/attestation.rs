pub struct Params<'a> {
    pub quote: &'a [u8],
    pub tcb_info: &'a [u8],
    pub tcb_sign_chain: &'a [u8],
    pub pck_certificate: &'a [u8],
    pub pck_sign_chain: &'a [u8],
    pub cert_revocation_list: &'a [u8],
    pub quoting_enclave_id: &'a [u8],
}
