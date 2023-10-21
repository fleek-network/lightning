use anyhow::anyhow;
use geohash::encode;

pub fn geohash(lat: f64, lon: f64) -> anyhow::Result<String> {
    let coord = geo_types::Coord { x: lon, y: lat };
    encode(coord, 7).map_err(|e| anyhow!(e))
}

#[cfg(test)]
mod tests {
    use crate::utils::geohash;

    #[tokio::test]
    async fn test_geohash() {
        let hash = geohash(0.0, 0.0).unwrap();
        assert_eq!(hash, "s000000");
    }
}
