use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {}

impl Default for Config {
    fn default() -> Self {
        Self {}
    }
}
