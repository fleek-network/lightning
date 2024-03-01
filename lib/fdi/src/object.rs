use std::any::{type_name, Any, TypeId};
use std::cell;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use crate::ty::Ty;

/// A registry entry. The box in this type is always a `Taker<T>` for some `T: 'static`.
pub struct Object(Ty, Box<dyn Any>);

impl Object {
    /// Create a new object with the given value.
    pub fn new<T: 'static>(value: T) -> Self {
        Self(Ty::of::<T>(), Box::new(Container::new(value)))
    }

    /// Return's the type id of the original value.
    pub fn ty(&self) -> &Ty {
        &self.0
    }

    /// Downcast this object to the requested type.
    pub fn downcast<T: 'static>(&self) -> &Container<T> {
        if self.0.id() != TypeId::of::<T>() {
            panic!(
                "Could not return object of type '{}' as '{}'.",
                self.0.name(),
                type_name::<T>()
            );
        }

        self.1.downcast_ref().unwrap()
    }

    /// Downcast this object to the requested type.
    pub fn downcast_into<T: 'static>(&self) -> &T {
        if self.0.container_id() != TypeId::of::<T>() {
            panic!(
                "Could not return object of type '{}' as '{}'.",
                self.0.name(),
                type_name::<T>()
            );
        }

        self.1.downcast_ref().unwrap()
    }
}

pub struct Container<T: 'static>(RefCell<Option<T>>);
pub struct Ref<'a, T: ?Sized + 'a>(pub(crate) RefInner<'a, T>);
pub struct RefMut<'a, T: ?Sized + 'a>(pub(crate) RefMutInner<'a, T>);
pub(crate) enum RefInner<'a, T: ?Sized + 'a> {
    Ref(&'a T),
    Cell(cell::Ref<'a, T>),
}
pub(crate) enum RefMutInner<'a, T: ?Sized + 'a> {
    Ref(&'a mut T),
    Cell(cell::RefMut<'a, T>),
}

impl<T: 'static> Container<T> {
    fn new(value: T) -> Self {
        Self(RefCell::new(Some(value)))
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        Ref(RefInner::Cell(
            cell::Ref::filter_map(self.0.borrow(), Option::as_ref)
                .unwrap_or_else(|_| panic!("The value is already taken.")),
        ))
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        RefMut(RefMutInner::Cell(
            cell::RefMut::filter_map(self.0.borrow_mut(), Option::as_mut)
                .unwrap_or_else(|_| panic!("The value is already taken.")),
        ))
    }

    pub fn take(&self) -> T {
        self.0
            .borrow_mut()
            .take()
            .unwrap_or_else(|| panic!("The value is already taken."))
    }
}

impl<'a, T: ?Sized + 'a> Deref for Ref<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            RefInner::Ref(a) => a,
            RefInner::Cell(a) => a,
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for RefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            RefMutInner::Ref(a) => a,
            RefMutInner::Cell(a) => a,
        }
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.0 {
            RefMutInner::Ref(a) => a,
            RefMutInner::Cell(a) => &mut *a,
        }
    }
}
