use std::collections::HashMap;

use indexmap::IndexSet;

use crate::dyn_method::DynMethod;
use crate::method::Method;
use crate::ty::Ty;
use crate::{MethodExt, Provider};

/// The [`Eventstore`] can be used to store a list of event handlers under each event name.
#[derive(Default)]
pub struct Eventstore {
    pub handlers: HashMap<&'static str, Vec<DynMethod<()>>>,
}

impl Eventstore {
    /// Extend the current event store with another event store.
    pub fn extend(&mut self, other: Eventstore) {
        for (ev, handlers) in other.handlers {
            self.handlers.entry(ev).or_default().extend(handlers);
        }
    }

    /// Return a set of all of the dependencies required to trigger an event.
    pub fn get_dependencies(&self, event: &'static str) -> IndexSet<Ty> {
        let mut result = IndexSet::new();

        if let Some(handlers) = self.handlers.get(event) {
            for handler in handlers {
                result.extend(handler.dependencies().iter());
            }
        }

        result
    }

    pub(crate) fn insert(&mut self, event: &'static str, handler: DynMethod<()>) {
        if handler.events().is_some() {
            panic!("event handler can not be a withevents.");
        }

        self.handlers.entry(event).or_default().push(handler);
    }

    /// Register a new handler for the given event. The handler will only be called once when the
    /// event is triggered.
    pub fn on<F, P>(&mut self, event: &'static str, handler: F)
    where
        F: Method<P>,
    {
        self.insert(event, DynMethod::new(handler.map(|_| ())))
    }

    /// Trigger the event. This is used internally.
    pub(crate) fn trigger(&mut self, event: &'static str, registry: &Provider) -> usize {
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
