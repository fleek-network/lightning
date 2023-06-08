use std::{fs, path::Path};

use anyhow::Context;
use draco_interfaces::config::ConfigProviderInterface;
use toml::Table;

/// The implementation of a configuration loader that uses the `toml` backend.
pub struct TomlConfigProvider {
    /// The [`ConfigProviderInterface`] does not put any constraints on the
    /// format of the document, except that we need a `[key: string]->any`
    /// mapping. The [`Table`] is that map.
    pub table: Table,
}

impl TomlConfigProvider {
    pub fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();

        let content = fs::read_to_string(path).with_context(|| {
            format!(
                "IO: Could not load the configuration file '{}'.",
                path.to_string_lossy()
            )
        })?;

        let table = toml::from_str::<Table>(&content).with_context(|| {
            format!(
                "Could not parse the configuration file '{}' as toml.",
                path.to_string_lossy()
            )
        })?;

        Ok(Self { table })
    }
}

impl ConfigProviderInterface for TomlConfigProvider {
    fn get<S: draco_interfaces::config::ConfigConsumer>(&self) -> S::Config {
        let key = S::KEY;
        self.table
            .get(key)
            .and_then(|v| v.clone().try_into().ok())
            .unwrap_or_default()
    }

    fn serialize_config(&self) -> String {
        todo!()
    }
}
