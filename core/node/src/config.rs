use std::{fs, path::Path, sync::Mutex};

use anyhow::Context;
use freek_interfaces::config::ConfigProviderInterface;
use toml::{Table, Value};

/// The implementation of a configuration loader that uses the `toml` backend.
#[derive(Default)]
pub struct TomlConfigProvider {
    /// The [`ConfigProviderInterface`] does not put any constraints on the
    /// format of the document, except that we need a `[key: string]->any`
    /// mapping. The [`Table`] is that map.
    pub table: Mutex<Table>,
}

impl TomlConfigProvider {
    pub fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();

        let table = if path.exists() {
            let content = fs::read_to_string(path).with_context(|| {
                format!(
                    "IO: Could not load the configuration file '{}'.",
                    path.to_string_lossy()
                )
            })?;

            toml::from_str::<Table>(&content).with_context(|| {
                format!(
                    "Could not parse the configuration file '{}' as toml.",
                    path.to_string_lossy()
                )
            })?
        } else {
            // If the file doesn't exist, use defaults for everything.
            Table::new()
        }
        .into();

        Ok(Self { table })
    }
}

impl ConfigProviderInterface for TomlConfigProvider {
    fn get<S: freek_interfaces::config::ConfigConsumer>(&self) -> S::Config {
        let mut table = self.table.lock().expect("failed to acquire lock");

        // Parse the table value into S::Config, or use the default in the event it doesn't exist.
        let item: S::Config = table
            .get(S::KEY)
            .and_then(|v| v.clone().try_into().ok())
            .unwrap_or_default();

        // Amend the internal table with the parsed or default item to be serialized later.
        table.insert(S::KEY.into(), Value::try_from(&item).unwrap());

        item
    }

    fn serialize_config(&self) -> String {
        toml::to_string(&self.table).expect("failed to serialize config")
    }
}
