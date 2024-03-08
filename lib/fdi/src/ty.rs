use std::any::{type_name, TypeId};
use std::fmt::Debug;
use std::hash::Hash;

/// A wrapper around [`TypeId`] that preserves the name.
#[derive(Clone, Copy)]
pub struct Ty {
    name: &'static str,
    ty: TypeId,
}

impl Ty {
    pub fn of<T: 'static>() -> Self {
        Self {
            name: type_name::<T>(),
            ty: TypeId::of::<T>(),
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
}

impl Debug for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Ty").field(&self.name).finish()
    }
}

// For correctness any equality check or hashing is perfomed only on .ty field because
// that is what actually matters. If in two different parts of the code the
// `type_name::<T>()` is different, it should not make the types also be different.

impl PartialEq for Ty {
    fn eq(&self, other: &Self) -> bool {
        self.ty.eq(&other.ty)
    }
}

impl Eq for Ty {}

impl PartialOrd for Ty {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ty {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ty.cmp(&other.ty)
    }
}

impl Hash for Ty {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ty.hash(state)
    }
}
