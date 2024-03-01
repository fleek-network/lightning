use std::any::{type_name, TypeId};
use std::fmt::Debug;

use crate::registry::Taker;

/// A wrapper around [`TypeId`] that preserves the name.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Ty {
    name: &'static str,
    type_id: TypeId,
    taker_id: TypeId,
}

impl Ty {
    pub fn of<T: 'static>() -> Self {
        Self {
            name: type_name::<T>(),
            type_id: TypeId::of::<T>(),
            taker_id: TypeId::of::<Taker<T>>(),
        }
    }

    /// Returns the original type id.
    pub fn id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the type id of `Taker<T>` where `T` was the original type id.
    pub fn taker_id(&self) -> TypeId {
        self.taker_id
    }
}

impl Debug for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Ty").field(&self.name).finish()
    }
}
