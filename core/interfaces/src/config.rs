use std::collections::HashMap;

use fdi::BuildGraph;
use lightning_firewall::Firewall;
use lightning_types::FirewallConfig;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::collection::Collection;

/// An implementer of this trait should handle providing the configurations from
/// the loaded configuration file.
#[interfaces_proc::blank]
pub trait ConfigProviderInterface<C: Collection>: BuildGraph + Send + Sync {
    /// Returns the configuration for the given object. If the key is not present
    /// in the loaded file we should return the default object.
    fn get<S: ConfigConsumer>(&self) -> S::Config;

    /// Create a [`Firewall`] object from the configuration of the given object.
    fn firewall<S>(&self) -> Firewall
    where
        S: ConfigConsumer,
        S::Config: Into<FirewallConfig>,
    {
        let config = self.get::<S>();

        Firewall::from_config(S::KEY, config.into())
    }

    /// Returns the textual representation of the configuration based on all values
    /// that have been loaded so far.
    fn serialize_config(&self) -> String;
}

/// Any object that in the program that is associated a configuration value
/// in the global configuration file.
#[interfaces_proc::blank]
pub trait ConfigConsumer {
    #[blank("BLANK")]
    /// The top-level key in the config file that should be used for this object.
    const KEY: &'static str;

    /// The type which is expected for this configuration object.
    #[blank(HashMap<String, String>)]
    type Config: Send + Sync + Serialize + DeserializeOwned + Default;
}
