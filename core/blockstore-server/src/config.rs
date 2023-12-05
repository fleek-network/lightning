use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    // Maximum number of concurrent peer requests we send out.
    pub max_conc_req: usize,
    // Maximum number of concurrent peer requests we respond to.
    pub max_conc_res: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_conc_req: 50,
            max_conc_res: 50,
        }
    }
}
