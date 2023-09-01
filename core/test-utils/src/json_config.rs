use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Mutex;

use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ConfigConsumer, ConfigProviderInterface};
use serde::Deserialize;
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

impl<C: Collection> ConfigProviderInterface<C> for JsonConfigProvider {
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
