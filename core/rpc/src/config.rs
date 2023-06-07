use draco_interfaces::ConfigProviderInterface;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Address to bind to
    pub addr: String,
    /// Port to listen on
    pub port: u16,
}
impl ConfigProviderInterface for Config {
    fn get<S: draco_interfaces::ConfigConsumer>(&self) -> S::Config {
        todo!()
    }

    fn serialize_config(&self) -> String {
        todo!()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1".to_owned(),
            port: 4069,
        }
    }
}
