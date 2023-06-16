use serde::{Deserialize, Serialize};

use crate::genesis::Genesis;

#[derive(Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Dev,
    Test,
    Prod,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub genesis: Option<Genesis>,
    pub mode: Mode,
}
