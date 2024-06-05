use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use lightning_interfaces::prelude::*;
use toml::{Table, Value};
use tracing::debug;

lazy_static! {
    pub static ref LIGHTNING_HOME_DIR: PathBuf = env::var("LIGHTNING_HOME")
        .unwrap_or("~/.lightning".to_string())
        .into();
}

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
    pub fn new() -> Self {
        Self {
            table: Table::new().into(),
            collection: PhantomData,
        }
    }

    pub fn inject<T: ConfigConsumer>(&self, config: T::Config) {
        let mut table = self.table.lock().expect("Failed to acquire lock");
        table.insert(T::KEY.to_owned(), Value::try_from(&config).unwrap());
    }

    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(anyhow!(
                "The configuration file '{}' does not exist. Run the `init` command to create it.",
                path.to_string_lossy()
            ));
        }

        let content = fs::read_to_string(path).with_context(|| {
            format!(
                "IO: Could not load the configuration file '{}'.",
                path.to_string_lossy()
            )
        })?;

        let table = toml::from_str::<Table>(&content)
            .with_context(|| {
                format!(
                    "Could not parse the configuration file '{}' as toml.",
                    path.to_string_lossy()
                )
            })?
            .into();

        Ok(Self {
            table,
            collection: PhantomData,
        })
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::write(&path, self.serialize_config()).with_context(|| {
            format!(
                "Could not write the configuration file: {}",
                path.as_ref().to_string_lossy()
            )
        })
    }

    pub fn into_inner(&self) -> Table {
        self.table.lock().unwrap().clone()
    }
}

impl<C: Collection> ConfigProviderInterface<C> for TomlConfigProvider<C> {
    fn get<S: lightning_interfaces::ConfigConsumer>(&self) -> S::Config {
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

impl<C: Collection> BuildGraph for TomlConfigProvider<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(|| Self::load("./config.toml"))
    }
}
