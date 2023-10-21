use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use maxminddb::geoip2::City;
use maxminddb::Reader;

use crate::utils;

pub struct IpInfo {
    pub geo: String,
    pub country: String,
    pub timezone: String,
}

#[derive(Clone)]
pub struct Maxmind {
    reader: Arc<Reader<Vec<u8>>>,
}

impl Maxmind {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(Self {
            reader: Arc::new(Reader::open_readfile(path)?),
        })
    }

    pub async fn get_ip_info(&self, address: String) -> Result<IpInfo> {
        let address: IpAddr = address.parse()?;
        let city = self.reader.lookup::<City>(address)?;
        let location = city.location.ok_or(anyhow!("missing location"))?;
        let latitude = location.latitude.ok_or(anyhow!("missing latitude"))?;
        let longitude = location.longitude.ok_or(anyhow!("missing longitude"))?;
        let geohash = utils::geohash(latitude, longitude)?;
        Ok(IpInfo {
            country: city
                .country
                .ok_or(anyhow!("missing country info"))?
                .iso_code
                .ok_or(anyhow!("missing country code"))?
                .to_string(),
            timezone: location
                .time_zone
                .ok_or(anyhow!("missing timezone"))?
                .to_string(),
            geo: geohash,
        })
    }
}
