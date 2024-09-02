use std::ffi::c_char;

use dcap_ql::Quote3Error;

// Linking with dcap prov
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct sgx_ql_qve_collateral_t {
    pub version: u32, // version = 1.  PCK Cert chain is in the Quote.
    pub pck_crl_issuer_chain: *mut c_char,
    pub pck_crl_issuer_chain_size: u32,
    pub root_ca_crl: *mut c_char, // Root CA CRL
    pub root_ca_crl_size: u32,
    pub pck_crl: *mut c_char, // PCK Cert CRL
    pub pck_crl_size: u32,
    pub tcb_info_issuer_chain: *mut c_char,
    pub tcb_info_issuer_chain_size: u32,
    pub tcb_info: *mut c_char, // TCB Info structure
    pub tcb_info_size: u32,
    pub qe_identity_issuer_chain: *mut c_char,
    pub qe_identity_issuer_chain_size: u32,
    pub qe_identity: *mut c_char, // QE Identity Structure
    pub qe_identity_size: u32,
}

#[link(name = "dcap_quoteprov")]
extern "C" {
    pub fn sgx_ql_get_quote_verification_collateral(
        fmspc: *const u8,
        fmspc_size: u16,
        pck_ra: *const c_char,
        pp_quote_collateral: *mut *mut sgx_ql_qve_collateral_t,
    ) -> Quote3Error;
    pub fn sgx_ql_free_quote_verification_collateral(
        p_quote_collateral: *const sgx_ql_qve_collateral_t,
    ) -> Quote3Error;
}
