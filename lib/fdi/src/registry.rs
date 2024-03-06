use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;

use crate::event::Eventstore;
use crate::object::{Object, Ref, RefMut};
use crate::ty::Ty;
use crate::Executor;

/// The registry contains all of the constructed values.
pub struct Registry {
    /// Map container id to object.
    values: HashMap<TypeId, Object>,
}

impl Registry {
    /// Insert the given value to the registry.
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.insert_raw(Object::new(value))
    }

    /// Insert the given object to the registry.
    pub fn insert_raw(&mut self, object: Object) {
        let ty = object.ty();
        self.values.insert(ty.container_id(), object);
    }

    /// Returns true if the registry has a value with the given type.
    pub fn contains<T: 'static>(&self) -> bool {
        self.contains_type_id(&Ty::of::<T>())
    }

    /// Returrn true if the given type id could be resolved using this registry.
    pub fn contains_type_id(&self, tid: &Ty) -> bool {
        self.values.contains_key(&tid.id()) || self.values.contains_key(&tid.container_id())
    }

    /// Returns a immutable reference to the value with given type from the registry
    ///
    /// In case of trying to get the container directly out of the registry this function is still
    /// supposed to work. In other words if type `T` has already been inserted in the registry, the
    /// request for `&Container<T>` will return the container to `T`.
    ///
    /// # Panics
    ///
    /// If the value with the given type is not inserted in the registry or if the value has been
    /// taken out of the container.
    pub fn get<T: 'static + Any>(&self) -> Ref<'_, T> {
        let ty = Ty::of::<T>();
        if let Some(obj) = self.values.get(&ty.id()) {
            // -> T: Container<U>
            Ref(crate::object::RefInner::Ref(obj.downcast_into()))
        } else if let Some(obj) = self.values.get(&ty.container_id()) {
            obj.downcast::<T>().borrow()
        } else {
            panic!(
                "Could not find a value of type '{}' in this registry.",
                type_name::<T>()
            )
        }
    }

    /// Return a mutable reference to the value of type `T` from the registry.
    ///
    /// # Panics
    ///
    /// If the value with the given type is not inserted in the registry or if the value has been
    /// taken out of the container. It also panics if `T: Container<U>`.
    pub fn get_mut<T: 'static + Any>(&self) -> RefMut<'_, T> {
        let ty = Ty::of::<T>();
        if self.values.get(&ty.id()).is_some() {
            // -> T: Container<U>
            panic!("Taking mutable reference to 'Container<_>' is not supported.")
        } else if let Some(obj) = self.values.get(&ty.container_id()) {
            obj.downcast::<T>().borrow_mut()
        } else {
            panic!(
                "Could not find a value of type '{}' in this registry.",
                type_name::<T>()
            )
        }
    }

    /// Consume the value of type `T` from the registry.
    ///
    /// # Panics
    ///
    /// If the value with the given type is not inserted in the registry or if the value has been
    /// taken out of the container. It also panics if `T: Container<U>`.
    pub fn take<T: 'static>(&self) -> T {
        let ty = Ty::of::<T>();
        if self.values.get(&ty.id()).is_some() {
            // -> T: Container<U>
            panic!("Consuming 'Container<_>' is not supported.")
        } else if let Some(obj) = self.values.get(&ty.container_id()) {
            obj.downcast::<T>().take()
        } else {
            panic!(
                "Could not find a value of type '{}' in this registry.",
                type_name::<T>()
            )
        }
    }

    /// Trigger the event with the given name from the event store.
    pub fn trigger(&self, event: &'static str) -> usize {
        self.get_mut::<Eventstore>().trigger(event, self)
    }
}

impl Default for Registry {
    fn default() -> Self {
        let mut tmp = Registry {
            values: HashMap::default(),
        };
        tmp.insert_raw(Object::new(Eventstore::default()));
        tmp.insert_raw(Object::new(Executor::default()));
        tmp
    }
}
