use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use indexmap::IndexSet;

use crate::method::{DynMethod, Method};
use crate::registry::Registry;
use crate::ty::Ty;

/// The [`Eventstore`] can be used to store a list of event handlers under each event name.
#[derive(Default)]
pub struct Eventstore {
    handlers: HashMap<&'static str, Vec<DynMethod>>,
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

    pub(crate) fn insert(&mut self, event: &'static str, handler: DynMethod) {
        if handler.events().is_some() {
            panic!("Event handler can not be a WithEvents.");
        }

        self.handlers.entry(event).or_default().push(handler);
    }

    /// Register a new handler for the given event. The handler will only be called once when the
    /// event is triggered.
    pub fn on<F, T, P>(&mut self, event: &'static str, handler: F)
    where
        F: Method<T, P>,
        T: 'static,
    {
        self.insert(event, DynMethod::new(handler))
    }

    /// Trigger the event. This is used internally.
    pub(crate) fn trigger(&mut self, event: &'static str, registry: &Registry) -> usize {
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

/// This wrapper allows to define a series of events on top of a method, and these events will only
/// be registerd (activated) when the method itself is called. In other words these events will not
/// even exist as long as the constructor method is used by the dependency graph.
pub struct WithEvents<F, T, P>
where
    F: Method<T, P>,
{
    method: F,
    events: RefCell<Eventstore>,
    _unused: PhantomData<(T, P)>,
}

impl<F, T, P> WithEvents<F, T, P>
where
    F: Method<T, P>,
{
    pub fn new(method: F) -> Self {
        Self {
            method,
            events: RefCell::new(Eventstore::default()),
            _unused: PhantomData,
        }
    }

    /// Register a new handler for the given event.
    pub fn on<Fx, Tx, Px>(self, event: &'static str, handler: Fx) -> Self
    where
        Fx: Method<Tx, Px>,
        Tx: 'static,
    {
        self.events.borrow_mut().on(event, handler);
        self
    }
}

impl<F, T, P> Method<T, P> for WithEvents<F, T, P>
where
    F: Method<T, P>,
{
    #[inline(always)]
    fn name(&self) -> &'static str {
        self.method.name()
    }

    #[inline(always)]
    fn display_name(&self) -> &'static str {
        self.method.display_name()
    }

    #[inline(always)]
    fn events(&self) -> Option<Eventstore> {
        let mut borrow = self.events.borrow_mut();
        let events_mut_ref = &mut *borrow;
        let events = std::mem::take(events_mut_ref);
        if let Some(mut prev) = self.method.events() {
            prev.extend(events);
            Some(prev)
        } else {
            Some(events)
        }
    }

    #[inline(always)]
    fn dependencies(&self) -> Vec<Ty> {
        self.method.dependencies()
    }

    #[inline(always)]
    fn call(self, registry: &Registry) -> T {
        self.method.call(registry)
    }
}
