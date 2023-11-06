use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Self {}
    }
}
