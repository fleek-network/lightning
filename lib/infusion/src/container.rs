use std::any::TypeId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;

use crate::error::CycleFound;
use crate::vtable::{self, Object, Tag, VTable};
use crate::{DependencyGraph, InitializationError};

/// The container contains an instance of every `<Type as Trait>`s.
#[derive(Default)]
pub struct Container {
    objects: HashMap<Tag, Object>,
}

impl Container {
    /// Provide the given value to the container. Should be used for
    /// for input traits.
    ///
    /// To generate a tag consider using the [`tag`](crate::tag) macro.
    pub fn with<T: 'static>(mut self, tag: Tag, value: T) -> Self {
        let type_id = TypeId::of::<T>();
        assert_eq!(tag.type_id(), type_id, "Type mismatch.");
        self.objects.insert(tag, Object::new(value));
        self
    }

    /// Returns a reference to the given value from the tag.
    pub fn get<T: 'static>(&self, tag: Tag) -> &T {
        self.objects
            .get(&tag)
            .expect("{tag:?} not initialized in the container.")
            .downcast::<T>()
    }

    /// Initialize the container based on the provided dependency graph.
    pub fn initialize(mut self, graph: DependencyGraph) -> Result<Self, InitializationError> {
        // Step 1: Ensure that every input type has been provided.
        for tag in graph.get_inputs() {
            if !self.objects.contains_key(tag) {
                return Err(InitializationError::InputNotProvided(*tag));
            }
        }

        // Step 2: Order the objects we need to initialize by topologically ordering the dependency
        // graph.
        let sorted = graph.sort().map_err(InitializationError::CycleFound)?;

        for tag in &sorted {
            if self.objects.contains_key(tag) {
                // If the object is already initialized before skip it.
                // This can happen if a project has several collections (unlikely,
                // but possible.). Or if a call to `with` has provided the object
                // before the initialization attempt.
                continue;
            }

            let vtable = graph.vtables.get(tag).unwrap();
            let object = vtable
                .init(&self)
                .map_err(|e| InitializationError::InitializationFailed(*tag, e))?;
            self.objects.insert(*tag, object);
        }

        for tag in sorted {
            let vtable = graph.vtables.get(&tag).unwrap();

            // We can not hold a mutable reference to self and pass &self to the
            // `post` function. Since `post` is not supposed to get reference to
            // self anyway, we can simply remove it from the map at this step and
            // put it back later.
            //
            // The edge case is that a trait would get `self` as a post dependency,
            // which is dumb enough for us to not care about supporting.

            let mut object = self.objects.remove(&tag).unwrap();
            vtable.post(&mut object, &self);
            self.objects.insert(tag, object);
        }

        // Run the `post_initialization` for the input types.
        for tag in graph.get_inputs() {
            let vtable = graph.vtables.get(tag).unwrap();
            let mut object = self.objects.remove(tag).unwrap();
            vtable.post(&mut object, &self);
            self.objects.insert(*tag, object);
        }

        Ok(self)
    }
}
