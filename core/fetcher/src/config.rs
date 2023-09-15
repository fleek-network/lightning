use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub max_concurrent_origin_requests: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_concurrent_origin_requests: 5,
        }
    }
}
