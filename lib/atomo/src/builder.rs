use std::{any::Any, hash::Hash, sync::Arc};

use dashmap::DashMap;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    db::{Atomo, TableId, UpdatePerm},
    inner::AtomoInner,
    serder::SerdeBackend,
    table::TableMeta,
    DefaultSerdeBackend,
};

pub struct AtomoBuilder<S: SerdeBackend = DefaultSerdeBackend> {
    atomo: AtomoInner<S>,
}

impl<S: SerdeBackend> AtomoBuilder<S> {
    #[must_use = "Creating a builder does not perform anything."]
    pub fn new() -> Self {
        Self {
            atomo: AtomoInner::empty(),
        }
    }

    /// Open a new table with the given name and key-value type.
    ///
    /// # Panics
    ///
    /// If another table with the given name is already created.
    #[must_use = "Builder is incomplete."]
    #[inline(always)]
    pub fn with_table<K, V>(mut self, name: impl ToString) -> Self
    where
        K: Hash + Eq + Serialize + DeserializeOwned + Any,
        V: Serialize + DeserializeOwned + Any,
    {
        let name = name.to_string();
        self.with_table_internal_non_generic_part(name.clone());
        self.atomo.tables.push(TableMeta::new::<K, V>(name));
        self
    }

    /// Performs the common operation for opening a table that is not depended
    /// on generic types to produce smaller code when `with_table` is inlined.
    fn with_table_internal_non_generic_part(&mut self, name: String) {
        let index = self.atomo.tables.len();

        if index > (TableId::MAX as usize) {
            panic!("Table ID overflow.");
        }

        if self
            .atomo
            .table_name_to_id
            .insert(name.clone(), index as TableId)
            .is_some()
        {
            panic!("Table {name} is already defined.");
        }

        self.atomo.persistence.push(DashMap::default());
    }

    #[must_use = "Creating a Atomo without using it is probably a mistake."]
    pub fn build(self) -> Atomo<UpdatePerm, S> {
        Atomo::new(Arc::new(self.atomo))
    }
}
