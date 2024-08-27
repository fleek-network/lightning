use std::io::{Read, Write};
use std::net::TcpStream;

use fleek_remote_attestation::types::collateral::SgxCollateral;
use sgx_isa::Report;

pub type TargetInfo = ();

/// Get the target info from the runner
pub fn get_target_info() -> std::io::Result<TargetInfo> {
    let bytes = request("target_info", None)?;
    todo!("parse bytes into target info")
}

/// Get a quote from the runner
pub fn get_quote(report: Report) -> std::io::Result<Vec<u8>> {
    let body = serde_json::to_string(&report)?;
    request("quote", Some(body.as_bytes()))
}

/// Get collateral from the runner
pub fn get_collateral(quote: Vec<u8>) -> std::io::Result<SgxCollateral> {
    let res = request("collateral", Some(&quote))?;
    let collat = serde_json::from_slice(&res)?;
    Ok(collat)
}

/// Request from the runner's attestation endpoint
fn request(method: &str, body: Option<&[u8]>) -> std::io::Result<Vec<u8>> {
    let mut conn = TcpStream::connect(method.to_string() + ".attest.fleek.network")?;
    if let Some(body) = body {
        conn.write_all(&(body.len() as u32).to_be_bytes())?;
        conn.write_all(body)?;
    }

    let mut buf = Vec::new();
    conn.read_to_end(&mut buf)?;
    Ok(buf)
}
