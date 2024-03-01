use std::any::{type_name, TypeId};
use std::fmt::Debug;

use crate::object::Container;

/// A wrapper around [`TypeId`] that preserves the name.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Ty {
    name: &'static str,
    ty: TypeId,
    container_ty: TypeId,
}

impl Ty {
    pub fn of<T: 'static>() -> Self {
        Self {
            name: type_name::<T>(),
            ty: TypeId::of::<T>(),
            container_ty: TypeId::of::<Container<T>>(),
        }
    }

    /// Returns the name of the original type.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the original type id.
    pub fn id(&self) -> TypeId {
        self.ty
    }

    /// Returns the type id of `Container<T>` where `T` was the original type id.
    pub fn container_id(&self) -> TypeId {
        self.container_ty
    }
}

impl Debug for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Ty").field(&self.name).finish()
    }
}
