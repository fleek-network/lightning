// from: https://github.com/fleek-network/ursa/blob/1e5800f03195b7d44d1263cbe61792ac3dcf68d5/crates/ursa-tracker/src/ip_api.rs

use anyhow::{anyhow, Result};
use geohash::encode;
use hyper::client::connect::dns::GaiResolver;
use hyper::service::Service;
use hyper::Client;
use hyper_tls::HttpsConnector;
use serde_derive::Deserialize;

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct IpInfoResponse {
    #[serde(rename = "ip")]
    pub addr: String,
    pub hostname: String,
    pub city: String,
    pub region: String,
    pub country: String,
    pub loc: String,
    pub org: String,
    pub postal: String,
    pub timezone: String,
    pub geo: String,
}

/// Get public ip info from https://ipinfo.io
pub async fn get_ip_info(token: &str, addr: String) -> Result<IpInfoResponse> {
    let mut dns = false;
    // attempt to resolve with dns if not an ip
    let ip = if !addr.is_empty() && addr.parse::<std::net::IpAddr>().is_err() {
        dns = true;
        GaiResolver::new()
            .call(addr.parse()?)
            .await?
            .next()
            .ok_or_else(|| anyhow!("No ip found"))?
            .ip()
            .to_string()
    } else {
        addr.clone()
    };

    let url = format!("https://ipinfo.io/{ip}?{token}");
    let client = Client::builder().build::<_, hyper::Body>(HttpsConnector::new());

    let res = client.get(url.parse()?).await?;
    let status = res.status();
    let data = hyper::body::to_bytes(res.into_body()).await?;

    if !status.is_success() {
        return Err(anyhow!("Failed to get ip info: {}", status));
    }

    let mut info: IpInfoResponse = serde_json::from_slice(&data)?;
    let loc = info.loc.split(',').collect::<Vec<&str>>();
    let lat = loc[0].parse::<f64>()?;
    let lon = loc[1].parse::<f64>()?;
    info.geo = geohash(lat, lon)?;
    if dns {
        info.addr = addr;
    }

    Ok(info)
}

pub fn geohash(lat: f64, lon: f64) -> Result<String> {
    let coord = geo_types::Coord { x: lon, y: lat };
    encode(coord, 7).map_err(|e| anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::{geohash, get_ip_info};

    fn token() -> String {
        std::env::var("IPINFO_TOKEN").expect("IPINFO_TOKEN is not set")
    }

    #[tokio::test]
    async fn test_geohash() {
        let hash = geohash(0.0, 0.0).unwrap();
        assert_eq!(hash, "s000000");
    }

    #[ignore = "requires api token"]
    #[tokio::test]
    async fn test_self_ip_info() {
        get_ip_info(&token(), "".to_string()).await.unwrap();
    }

    #[ignore = "requires api token"]
    #[tokio::test]
    async fn test_remote_ip_info() {
        get_ip_info(&token(), "8.8.8.8".to_string()).await.unwrap();
    }

    #[ignore = "requires api token"]
    #[tokio::test]
    async fn test_dns_info() {
        get_ip_info(&token(), "google.com".into()).await.unwrap();
    }
}
