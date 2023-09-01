use std::any::Any;
use std::hash::Hash;
use std::iter::Extend;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::db::{Atomo, TableId, UpdatePerm};
use crate::inner::AtomoInner;
use crate::serder::SerdeBackend;
use crate::storage::{InMemoryStorage, StorageBackendConstructor};
use crate::table::TableMeta;
use crate::{DefaultSerdeBackend, StorageBackend};

/// The builder API to use for opening an [`Atomo`] database.
pub struct AtomoBuilder<
    B: StorageBackendConstructor = InMemoryStorage,
    S: SerdeBackend = DefaultSerdeBackend,
> {
    constructor: B,
    atomo: AtomoInner<(), S>,
}

impl<B: StorageBackendConstructor, S: SerdeBackend> AtomoBuilder<B, S> {
    /// Create an empty builder.
    #[must_use = "Creating a builder does not perform anything."]
    pub fn new(constructor: B) -> Self {
        Self {
            constructor,
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

        self.constructor.open_table(name);
    }

    /// Enable the iterator functionality on the provided table. In Atomo by default
    /// tables do not have a key iterator, to implement a non-blocking iterator over
    /// keys we currently store all of the keys in memory, this may not be the best
    /// we can do, but it suffices the needs we have.
    ///
    /// So do not enable the iterator on tables that you don't need it on.
    ///
    /// # Panics
    ///
    /// Panics if the provided table name is not already defined using a prior call
    /// to `with_table`.
    #[must_use = "Builder is incomplete."]
    pub fn enable_iter(mut self, name: &str) -> Self {
        if let Some(index) = self.atomo.table_name_to_id.get(name) {
            self.atomo
                .snapshot_list
                .get_metadata_mut()
                .enable(*index as usize);
            return self;
        }

        panic!("Table {name} is not defined.");
    }

    /// Finish the construction and returns an [`Atomo`] with [`UpdatePerm`] permission.
    #[must_use = "Creating a Atomo without using it is probably a mistake."]
    pub fn build(self) -> Result<Atomo<UpdatePerm, B::Storage, S>, B::Error> {
        Ok(Atomo::new(Arc::new(self.build_inner()?)))
    }

    /// Build and return the internal [`AtomoInner`]. Used for testing purposes.
    pub(crate) fn build_inner(mut self) -> Result<AtomoInner<B::Storage, S>, B::Error> {
        let storage = self.constructor.build()?;

        // Upon opening read every key for the tables that have enabled the
        // iterator.
        //
        // The `vertical_keys.update` method only runs the closure if the table
        // has already seen a prior `enable_iter` call.
        //
        // So we just iterate through every table and attempt to *update* the
        // list of keys if present.

        let vertical_keys = self.atomo.snapshot_list.get_metadata_mut();

        let count = self.atomo.tables.len() as u8;

        for tid in 0..count {
            vertical_keys.update(tid, |value| {
                // TODO(qti3e): The extend method here does not do anything smart and just does
                // several inserts and each insert is O(log n). And we know this is the initial
                // change and nothing is referring to this im instance. So.. we can do better.
                value.extend(storage.keys(tid).into_iter())
            });
        }

        Ok(self.atomo.swap_persistance(storage))
    }
}

impl<B: StorageBackendConstructor, S: SerdeBackend> Default for AtomoBuilder<B, S>
where
    B: Default,
{
    #[must_use = "Creating a builder does not perform anything."]
    fn default() -> Self {
        Self::new(B::default())
    }
}
