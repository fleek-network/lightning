use std::any::{type_name, Any, TypeId};

use fxhash::FxHashMap;

#[derive(Default)]
pub struct TypedStorage {
    data: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypedStorage {
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        let tid = TypeId::of::<T>();
        let item = Box::new(value);
        if self.data.insert(tid, item).is_some() {
            panic!(
                "Another item with type '{}' has already been inserted.",
                type_name::<T>()
            );
        }
    }

    pub fn with<F, T: Any, U>(&self, closure: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let tid = TypeId::of::<T>();
        let item = self.data.get(&tid).unwrap_or_else(|| {
            panic!(
                "No value for item with type '{}' can be found.",
                type_name::<T>()
            )
        });
        let value = item.downcast_ref::<T>().unwrap();
        closure(value)
    }
}
