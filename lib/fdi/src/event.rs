use std::any::TypeId;
use std::collections::HashMap;

use indexmap::IndexSet;

use crate::method::{DynMethod, Method};
use crate::registry::Registry;

/// The [`Eventstore`] can be used to store a list of event handlers under each event name.
#[derive(Default)]
pub struct Eventstore {
    handlers: HashMap<&'static str, Vec<DynMethod>>,
}

impl Eventstore {
    /// Return a set of all of the dependencies required to trigger an event.
    #[allow(unused)]
    fn get_dependencies(&self, event: &'static str) -> IndexSet<TypeId> {
        let mut result = IndexSet::new();

        if let Some(handlers) = self.handlers.get(event) {
            for handler in handlers {
                result.extend(handler.dependencies().iter());
            }
        }

        result
    }

    pub fn on<F, T, P>(&mut self, event: &'static str, handler: F)
    where
        F: Method<T, P>,
        T: 'static,
    {
        self.handlers
            .entry(event)
            .or_default()
            .push(DynMethod::new(handler));
    }

    pub fn trigger(&mut self, event: &'static str, registry: &Registry) -> usize {
        let mut result = 0;
        if let Some(handlers) = self.handlers.remove(event) {
            for handler in handlers {
                result += 1;
                handler.call(registry);
            }
        }
        result
    }
}
