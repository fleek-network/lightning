use std::io::{Read, Write};
use std::net::TcpStream;

use fleek_remote_attestation::types::collateral::SgxCollateral;
use sgx_isa::{Report, Targetinfo};

/// Generate a quote and collateral for a given report data slice
pub fn generate_for_report_data(data: [u8; 64]) -> std::io::Result<(Vec<u8>, SgxCollateral)> {
    let report = report_for_target(data)?;
    let quote = get_quote(report)?;
    let collateral = get_collateral(&quote)?;
    Ok((quote, collateral))
}

/// Generate a report for the quote target and given data
pub fn report_for_target(data: [u8; 64]) -> std::io::Result<Report> {
    let ti = get_target_info()?;
    Ok(Report::for_target(&ti, &data))
}

/// Get the target info from the runner
pub fn get_target_info() -> std::io::Result<Targetinfo> {
    let res = request("target_info", None)?;
    let ti = serde_json::from_slice(&res)?;
    Ok(ti)
}

/// Get a quote from the runner
pub fn get_quote(report: Report) -> std::io::Result<Vec<u8>> {
    let body = serde_json::to_string(&report)?;
    request("quote", Some(body.as_bytes()))
}

/// Get collateral from the runner
pub fn get_collateral(quote: &[u8]) -> std::io::Result<SgxCollateral> {
    let res = request("collateral", Some(quote))?;
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
