use std::any::{type_name, Any, TypeId};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::rc_cell::{self, RcCell};
use crate::ty::Ty;

/// A registry entry. The box in this type is always a `Taker<T>` for some `T: 'static`.
pub(crate) struct Object(Ty, Box<dyn Any>);

impl Object {
    /// Create a new object with the given value.
    pub fn new<T: 'static>(value: T) -> Self {
        Self(Ty::of::<T>(), Box::new(Container::new(value)))
    }

    /// Return's the type id of the original value.
    pub fn ty(&self) -> &Ty {
        &self.0
    }

    /// Downcast this object to the requested type. Note that upon creation of an object with
    /// type `T` it is wrapped in a `Container<T>` so you can only downcast to `Container<T>` and
    /// not `T` directly.
    pub fn downcast<T: 'static>(&self) -> &T {
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

pub struct Container<T: 'static>(RcCell<Option<T>>);
pub struct Ref<T: 'static>(pub(crate) RefInner<T>);
pub struct RefMut<T: 'static>(pub(crate) rc_cell::RefMut<Option<T>>);
pub(crate) enum RefInner<T: 'static> {
    Ref(Rc<Object>),
    Cell(rc_cell::Ref<Option<T>>),
}

impl<T: 'static> Container<T> {
    fn new(value: T) -> Self {
        Self(RcCell::new(Some(value)))
    }

    pub fn borrow(&self) -> Ref<T> {
        let cell_ref = self.0.borrow();
        if cell_ref.is_none() {
            panic!("The value is already taken.");
        }

        Ref(RefInner::Cell(cell_ref))
    }

    pub fn borrow_mut(&self) -> RefMut<T> {
        let cell_ref = self.0.borrow_mut();
        if cell_ref.is_none() {
            panic!("The value is already taken.");
        }
        RefMut(cell_ref)
    }

    pub fn take(&self) -> T {
        self.0
            .borrow_mut()
            .take()
            .unwrap_or_else(|| panic!("The value is already taken."))
    }
}

impl<T: 'static> Deref for Ref<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            RefInner::Ref(a) => a.downcast(),
            RefInner::Cell(a) => a.as_ref().unwrap(),
        }
    }
}

impl<T: 'static> Deref for RefMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl<T: 'static> DerefMut for RefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_downcast_should_work() {
        let obj = Object::new(String::from("Hello"));
        let ty = Ty::of::<String>();
        assert_eq!(obj.ty(), &ty);
        let container = obj.downcast::<Container<String>>();
        let s = &*container.borrow();
        assert_eq!(s, &String::from("Hello"));
        let container = obj.downcast::<Container<String>>();
        let s = &*container.borrow();
        assert_eq!(s, &String::from("Hello"));
    }

    #[test]
    #[should_panic]
    fn downcast_to_org_should_panic() {
        let obj = Object::new(String::from("Hello"));
        let ty = Ty::of::<String>();
        assert_eq!(obj.ty(), &ty);
        obj.downcast::<String>();
    }

    #[test]
    fn container() {
        let container = Container::new(String::from("Hello"));
        let a0 = &*container.borrow();
        let a1 = &*container.borrow();
        assert_eq!(a0, a1);
    }

    #[test]
    #[should_panic]
    fn container_borrow_and_borrow_mut() {
        let container = Container::new(String::from("Hello"));
        let a0 = container.borrow();
        let a1 = container.borrow_mut();
        drop((a0, a1));
    }

    #[test]
    #[should_panic]
    fn container_take_and_borrow() {
        let container = Container::new(String::from("Hello"));
        let a0 = container.take();
        let a1 = container.borrow();
        drop((a0, a1));
    }

    #[test]
    #[should_panic]
    fn container_take_and_borrow_mut() {
        let container = Container::new(String::from("Hello"));
        let a0 = container.take();
        let a1 = container.borrow_mut();
        drop((a0, a1));
    }

    #[test]
    #[should_panic]
    fn container_take_and_take() {
        let container = Container::new(String::from("Hello"));
        let a0 = container.take();
        let a1 = container.take();
        drop((a0, a1));
    }
}
