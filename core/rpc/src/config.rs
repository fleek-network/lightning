use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Address to bind to
    pub addr: String,
    /// Port to listen on
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1".to_owned(),
            port: 4069,
        }
    }
}
