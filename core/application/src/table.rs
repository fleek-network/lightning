use std::{any::Any, cell::RefCell, hash::Hash};

use atomo::{KeyIterator, SerdeBackend, TableRef as AtomoTableRef, TableSelector};
use serde::{de::DeserializeOwned, Serialize};

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

pub struct StateTables<'selector, S: SerdeBackend> {
    pub table_selector: &'selector TableSelector<S>,
}

impl<'selector, S: SerdeBackend> Backend for StateTables<'selector, S> {
    type Ref<
        K: Eq + Hash + Send + Serialize + DeserializeOwned + 'static,
        V: Clone + Send + Serialize + DeserializeOwned + 'static,
    > = AtomoTable<'selector, K, V, S>;

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
    S: SerdeBackend,
>(RefCell<AtomoTableRef<'selector, K, V, S>>);

impl<
    'selector,
    K: Hash + Eq + Serialize + DeserializeOwned + Any,
    V: Serialize + DeserializeOwned + Any + Clone,
    S: SerdeBackend,
> TableRef<K, V> for AtomoTable<'selector, K, V, S>
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
