use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    // Maximum number of concurrent origin requests we send out.
    pub max_conc_origin_req: usize,
    // Maximum number of concurrent peer requests we send out.
    pub max_conc_req: usize,
    // Maximum number of concurrent peer requests we respond to.
    pub max_conc_res: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_conc_origin_req: 5,
            max_conc_req: 5,
            max_conc_res: 5,
        }
    }
}
