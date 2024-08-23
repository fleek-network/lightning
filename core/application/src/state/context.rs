use std::any::Any;
use std::cell::RefCell;
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
}
