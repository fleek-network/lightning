use std::fs;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::ConfigConsumer;
use resolved_pathbuf::ResolvedPathBuf;
use toml::{Table, Value};
use tracing::debug;

/// The implementation of a configuration loader that uses the `toml` backend.
pub struct TomlConfigProvider<C: Collection> {
    /// The [`ConfigProviderInterface`] does not put any constraints on the
    /// format of the document, except that we need a `[key: string]->any`
    /// mapping. The [`Table`] is that map.
    table: Mutex<Table>,
    collection: PhantomData<C>,
}

impl<C: Collection> Clone for TomlConfigProvider<C> {
    fn clone(&self) -> Self {
        let guard = self.table.lock().expect("Failed to lock.");
        let table = guard.clone();
        Self {
            table: Mutex::new(table),
            collection: PhantomData,
        }
    }
}

impl<C: Collection> Default for TomlConfigProvider<C> {
    fn default() -> Self {
        Self {
            table: Default::default(),
            collection: PhantomData,
        }
    }
}

impl<C: Collection> TomlConfigProvider<C> {
    pub fn inject<T: ConfigConsumer>(&self, config: T::Config) {
        let mut table = self.table.lock().expect("Failed to acquire lock");
        table.insert(T::KEY.to_owned(), Value::try_from(&config).unwrap());
    }

    fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
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

        Ok(Self {
            table,
            collection: PhantomData,
        })
    }

    pub async fn load_or_write_config(
        config_path: ResolvedPathBuf,
    ) -> Result<TomlConfigProvider<C>> {
        let config = Self::open(&config_path)?;
        Node::<C>::fill_configuration(&config);

        if !config_path.exists() {
            std::fs::write(&config_path, config.serialize_config()).with_context(|| {
                format!("Could not write to file: {}", config_path.to_str().unwrap())
            })?;
        }

        Ok(config)
    }

    pub fn into_inner(&self) -> Table {
        self.table.lock().unwrap().clone()
    }
}

impl<C: Collection> ConfigProviderInterface<C> for TomlConfigProvider<C> {
    fn get<S: lightning_interfaces::config::ConfigConsumer>(&self) -> S::Config {
        debug!("Getting the config for {}", std::any::type_name::<S>());

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
