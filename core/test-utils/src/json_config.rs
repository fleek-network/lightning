use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Mutex;

use lightning_interfaces::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{to_value, Value};

#[derive(Default)]
pub struct JsonConfigProvider {
    value: Mutex<HashMap<String, Value>>,
}

impl From<Value> for JsonConfigProvider {
    fn from(value: Value) -> Self {
        let map = HashMap::deserialize(value).expect("expected a map");
        Self {
            value: Mutex::new(map),
        }
    }
}

impl JsonConfigProvider {
    /// Insert a configuration into the config, returning the previous entry if it existed.
    #[inline(always)]
    pub fn insert<K: Into<String>, V: Serialize>(
        &mut self,
        key: K,
        value: V,
    ) -> Option<serde_json::Value> {
        self.value
            .lock()
            .unwrap()
            .insert(key.into(), to_value(value).unwrap())
    }

    /// Helper to inline an insertion for a config consumer during construction
    #[inline(always)]
    pub fn with<T: ConfigConsumer>(mut self, value: T::Config) -> Self {
        if self.insert(T::KEY, value).is_some() {
            panic!("attempted to insert duplicate entry for {}", T::KEY)
        }

        self
    }
}

impl<C: NodeComponents> ConfigProviderInterface<C> for JsonConfigProvider {
    fn get<S: ConfigConsumer>(&self) -> S::Config {
        let mut guard = self.value.lock().unwrap();
        match guard.entry(S::KEY.to_string()) {
            Entry::Occupied(e) => S::Config::deserialize(e.get()).expect("invalid value"),
            Entry::Vacant(e) => {
                let default = S::Config::default();
                let value = to_value(&default).expect("failed to serialize default config");
                e.insert(value);
                default
            },
        }
    }

    fn serialize_config(&self) -> String {
        let guard = self.value.lock().unwrap();
        let map = serde_json::to_value(&*guard).unwrap();
        format!("{:#}", map)
    }
}

impl BuildGraph for JsonConfigProvider {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(Self::default)
    }
}
