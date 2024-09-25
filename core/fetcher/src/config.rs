use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    // Maximum number of concurrent origin requests we send out.
    pub max_conc_origin_req: usize,
    pub http: lightning_origin_http::Config,
    pub ipfs: lightning_origin_ipfs::Config,
    pub b3fs: lightning_origin_b3fs::Config,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_conc_origin_req: 5,
            http: lightning_origin_http::Config::default(),
            ipfs: lightning_origin_ipfs::Config::default(),
            b3fs: lightning_origin_b3fs::Config::default(),
        }
    }
}
