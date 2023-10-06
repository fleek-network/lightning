use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    // Maximum number of concurrent origin requests we send out.
    pub max_conc_origin_req: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_conc_origin_req: 5,
        }
    }
}
