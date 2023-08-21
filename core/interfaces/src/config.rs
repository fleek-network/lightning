use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::infu_collection::Collection;

/// An implementer of this trait should handle providing the configurations from
/// the loaded configuration file.
#[infusion::service]
pub trait ConfigProviderInterface<C: Collection>: Send + Sync {
    /// Returns the configuration for the given object. If the key is not present
    /// in the loaded file we should return the default object.
    fn get<S: ConfigConsumer>(&self) -> S::Config;

    /// Returns the textual representation of the configuration based on all values
    /// that have been loaded so far.
    fn serialize_config(&self) -> String;
}

/// Any object that in the program that is associated a configuration value
/// in the global configuration file.
#[infusion::blank]
pub trait ConfigConsumer {
    /// The top-level key in the config file that should be used for this object.
    const KEY: &'static str = "BLANK";

    /// The type which is expected for this configuration object.
    type Config: Serialize + DeserializeOwned + Default = HashMap<String, String>;
}
