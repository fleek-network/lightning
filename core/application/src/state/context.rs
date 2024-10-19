use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;

use atomo::{KeyIterator, SerdeBackend, StorageBackend, TableRef as AtomoTableRef, TableSelector};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub trait Backend {
    type Ref<K: Eq + Hash + Send + Serialize + DeserializeOwned
     + 'static, V: Clone + Send + Serialize + DeserializeOwned + 'static>: TableRef<K, V>;

    fn get_table_reference<
        K: Eq + Hash + Send + Serialize + DeserializeOwned,
        V: Clone + Send + Serialize + DeserializeOwned,
    >(
        &self,
        id: &str,
    ) -> Self::Ref<K, V>;
}

pub trait TableRef<K, V> {
    fn set(&self, key: K, value: V);
    fn get(&self, key: &K) -> Option<V>;
    fn keys(&self) -> KeyIterator<K>;
    fn remove(&self, key: &K);
    fn clear(&self);
    fn as_map(&self) -> HashMap<K, V>;
}

pub struct StateContext<'selector, B: StorageBackend, S: SerdeBackend> {
    pub table_selector: &'selector TableSelector<B, S>,
}

impl<'selector, B: StorageBackend, S: SerdeBackend> Backend for StateContext<'selector, B, S> {
    type Ref<
        K: Eq + Hash + Send + Serialize + DeserializeOwned + 'static,
        V: Clone + Send + Serialize + DeserializeOwned + 'static,
    > = AtomoTable<'selector, K, V, B, S>;

    fn get_table_reference<
        K: Eq + Hash + Send + Serialize + DeserializeOwned,
        V: Clone + Send + Serialize + DeserializeOwned,
    >(
        &self,
        id: &str,
    ) -> Self::Ref<K, V> {
        AtomoTable(RefCell::new(self.table_selector.get_table(id)))
    }
}

pub struct AtomoTable<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + 'static,
    V: Serialize + DeserializeOwned + 'static,
    B: StorageBackend,
    S: SerdeBackend,
>(RefCell<AtomoTableRef<'selector, K, V, B, S>>);

impl<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any + Clone,
    B: StorageBackend,
    S: SerdeBackend,
> TableRef<K, V> for AtomoTable<'selector, K, V, B, S>
{
    fn set(&self, key: K, value: V) {
        self.0.borrow_mut().insert(key, value);
    }

    fn get(&self, key: &K) -> Option<V> {
        self.0.borrow_mut().get(key)
    }

    fn keys(&self) -> KeyIterator<K> {
        self.0.borrow_mut().keys()
    }

    fn remove(&self, key: &K) {
        self.0.borrow_mut().remove(key)
    }

    fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    fn as_map(&self) -> HashMap<K, V> {
        self.0.borrow().as_map()
    }
}

#[cfg(test)]
mod tests {
    use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, InMemoryStorage, UpdatePerm};

    use super::*;

    #[test]
    fn test_get_set() {
        let mut db = new_test_db();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.set("key1".to_string(), "value1".to_string());
            assert_eq!(table.get(&"key1".to_string()), Some("value1".to_string()));
        });
    }

    #[test]
    fn test_remove() {
        let mut db = new_test_db();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.set("key1".to_string(), "value1".to_string());
            table.remove(&"key1".to_string());
            assert_eq!(table.get(&"key1".to_string()), None);
        });
    }

    #[test]
    fn test_keys() {
        let mut db = new_test_db_with_enable_iter();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.set("key1".to_string(), "value1".to_string());
            table.set("key2".to_string(), "value2".to_string());
            assert_eq!(table.keys().count(), 2);
            assert_eq!(table.keys().collect::<Vec<_>>(), vec!["key1", "key2"]);
        });
    }

    #[test]
    #[should_panic]
    fn test_keys_should_panic_without_enable_iter() {
        let mut db = new_test_db();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.keys();
        });
    }

    #[test]
    fn test_clear() {
        let mut db = new_test_db_with_enable_iter();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.set("key1".to_string(), "value1".to_string());
            table.set("key2".to_string(), "value2".to_string());
            table.clear();
            assert_eq!(table.keys().count(), 0);
        });
    }

    #[test]
    #[should_panic]
    fn test_clear_should_panic_without_enable_iter() {
        let mut db = new_test_db();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.clear();
        });
    }

    #[test]
    fn test_as_map() {
        let mut db = new_test_db_with_enable_iter();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.set("key1".to_string(), "value1".to_string());
            table.set("key2".to_string(), "value2".to_string());
            assert_eq!(
                table.as_map(),
                HashMap::from([
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string())
                ])
            );
        });
    }

    #[test]
    #[should_panic]
    fn test_as_map_should_panic_without_enable_iter() {
        let mut db = new_test_db();
        db.run(|ctx| {
            let ctx = StateContext {
                table_selector: ctx,
            };
            let table = ctx.get_table_reference::<String, String>("data");
            table.as_map();
        });
    }

    fn new_test_db() -> Atomo<UpdatePerm, InMemoryStorage, DefaultSerdeBackend> {
        AtomoBuilder::<_, DefaultSerdeBackend>::new(InMemoryStorage::default())
            .with_table::<String, String>("data")
            .build()
            .unwrap()
    }

    fn new_test_db_with_enable_iter() -> Atomo<UpdatePerm, InMemoryStorage, DefaultSerdeBackend> {
        AtomoBuilder::<_, DefaultSerdeBackend>::new(InMemoryStorage::default())
            .with_table::<String, String>("data")
            .enable_iter("data")
            .build()
            .unwrap()
    }
}
