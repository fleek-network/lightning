use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};

use crate::dyn_method::DynMethod;
use crate::method::Method;
use crate::provider::{to_obj, Object};
use crate::ty::{Param, Ty};
use crate::{Eventstore, MethodExt, Provider};

#[derive(Default)]
pub struct DependencyGraph {
    touched: bool,
    constructors: HashMap<Ty, DynMethod<Result<Object>>>,
    pub(crate) graph: IndexMap<Ty, IndexSet<Param>>,
    ordered: Rc<Vec<Ty>>,
}

/// The dependency graph should be used to create a project model. This can be done by providing
/// a set of constructors for different values.
impl DependencyGraph {
    /// Create a new and empty [DependencyGraph].
    pub fn new() -> Self {
        Self::default()
    }

    pub fn viz(&self, title: &'static str) -> String {
        crate::viz::dependency_graph(self, title)
    }

    /// Expand the current dependency graph from another dependency graph.
    ///
    /// # Panics
    ///
    /// If any of the items in `other` is already present in this graph.
    pub fn expand(mut self, other: DependencyGraph) -> Self {
        for (tid, method) in other.constructors {
            self.insert(tid, method);
        }
        self
    }

    /// Shorthand function to use the [`default`](Default::default) as the constructor for a value.
    ///
    /// # Panics
    ///
    /// If there is already another constructor to provide this type.
    ///
    /// # Example
    ///
    /// ```
    /// use fdi::*;
    ///
    /// struct Value(String);
    ///
    /// impl Default for Value {
    ///     fn default() -> Self {
    ///         Value(String::from("Fleek"))
    ///     }
    /// }
    ///
    /// let graph = DependencyGraph::new().with_default::<Value>();
    /// let mut provider = Provider::default();
    /// graph.init_all(&mut provider).unwrap();
    /// assert_eq!(&*provider.get::<Value>().0, "Fleek");
    /// ```
    pub fn with_default<T>(self) -> Self
    where
        T: 'static + Default,
    {
        self.with(|| Ok(T::default()))
    }

    /// Provides the graph with an already pre-computed value for a given type.
    ///
    /// # Panics
    ///
    /// If there is already another constructor to provide this type.
    ///
    /// # Example
    ///
    /// ```
    /// use fdi::*;
    ///
    /// let graph = DependencyGraph::new().with_value(String::from("Hello!"));
    /// let mut provider = Provider::default();
    /// graph.init_all(&mut provider).unwrap();
    /// assert_eq!(&*provider.get::<String>(), "Hello!");
    /// ```
    pub fn with_value<T>(self, value: T) -> Self
    where
        T: 'static,
    {
        self.with(|| (Ok(value)))
    }

    /// Provide the graph with an infalliable constructor for a certain type. It is expected for a
    /// constructor of type `T` to provide a value of type `T` directly, without wrapping it in an
    /// anyhow [`Result`].
    ///
    /// # Panics
    ///
    /// If there is already another constructor to provide this type.
    ///
    /// # Example
    ///
    /// ```
    /// use fdi::*;
    /// let graph = DependencyGraph::new().with_infallible(|| String::from("Fleek"));
    /// let mut provider = Provider::default();
    /// graph.init_all(&mut provider).unwrap();
    /// assert_eq!(&*provider.get::<String>(), "Fleek");
    /// ```
    pub fn with_infallible<F, P>(self, f: F) -> Self
    where
        F: Method<P>,
        F::Output: 'static,
    {
        self.with(f.to_infallible())
    }

    /// Provide the graph with a failable constructor for a certain type. The output type of the
    /// provided method must be `Result<T>`, if an error is returned during the initialization it
    /// will be returned.
    ///
    /// # Panics
    ///
    /// If there is already another constructor to provide this type.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::{bail, Result};
    /// use fdi::*;
    /// let graph = DependencyGraph::new().with(|| -> Result<String> { bail!("error") });
    /// let mut provider = Provider::default();
    /// assert!(graph.init_all(&mut provider).is_err());
    ///
    /// let graph = DependencyGraph::new().with(|| Ok(String::from("Fleek")));
    /// let mut provider = Provider::default();
    /// graph.init_all(&mut provider).unwrap();
    /// assert_eq!(&*provider.get::<String>(), "Fleek");
    /// ```
    pub fn with<F, T, P>(mut self, f: F) -> Self
    where
        F: Method<P, Output = Result<T>>,
        T: 'static,
    {
        self.touched = true;
        self.insert(Ty::of::<T>(), DynMethod::new(f.map(|v| v.map(to_obj))));
        self
    }

    /// Internal method to insert a constructor method to this graph.
    fn insert(&mut self, tid: Ty, method: DynMethod<Result<Object>>) {
        debug_assert_eq!(method.ty(), Ty::of::<Result<Object>>());

        // TODO(qti3e): Right now this is a hack for our Blank use case. We should panic
        // here eventually.
        if self.constructors.contains_key(&tid) {
            return;
        }

        self.touched = true;

        if let Some(old) = self.constructors.get(&tid) {
            panic!(
                "A constructor for type '{}' is already present.\n\told='{}'\n\tnew='{}'",
                method.ty().name(),
                old.name(),
                method.name()
            );
        }

        self.graph
            .insert(tid, method.dependencies().iter().copied().collect());
        self.constructors.insert(tid, method);
    }

    #[inline]
    fn capture_events_from_already_constructed(&mut self, provider: &mut Provider) {
        let mut collect = Vec::new();
        for ty in self.constructors.keys() {
            if provider.contains_ty(ty) {
                collect.push(*ty);
            }
        }
        let mut events = provider.get_mut::<Eventstore>();
        for ty in collect {
            let method = self.constructors.remove(&ty).unwrap();
            if let Some(e) = method.events() {
                events.extend(e);
            }
        }
    }

    fn ensure_topo_order(&mut self) {
        if !self.touched {
            return;
        }

        self.touched = false;
        let mut result = Vec::new();

        // Nodes with degree == 0.
        let len = self.graph.len();
        let mut queue = VecDeque::<Ty>::with_capacity(len);

        // Map each node to its in-degree.
        let mut in_degree = IndexMap::<Ty, usize>::with_capacity(len);

        for (v, connections) in &self.graph {
            in_degree.entry(*v).or_default();

            for (_, tag) in connections {
                *in_degree.entry(*tag).or_default() += 1;
            }
        }

        for (tag, degree) in self
            .graph
            .keys()
            .filter_map(|t| in_degree.get(t).map(|v| (*t, *v)))
        {
            if degree == 0 {
                queue.push_back(tag);
            }
        }

        while let Some(u) = queue.pop_front() {
            // The degree is zero so it is not depended on any pending things anymore.
            result.push(u);

            // Remove it from the in_degree so that we can end up with only the
            // pending items once the queue is empty. (That would mean there is
            // a cycle)
            in_degree.remove(&u);

            if let Some(values) = self.graph.get(&u) {
                for (_, v) in values {
                    if let Some(ref_mut) = in_degree.get_mut(v) {
                        assert_ne!(*ref_mut, 0);

                        *ref_mut -= 1;

                        if *ref_mut == 0 {
                            queue.push_back(*v);
                        }
                    }
                }
            }
        }

        if !in_degree.is_empty() {
            // There is at least a cycle. We know it only involves the pending nodes.
            // We want to report each cycle separately.
            unimplemented!()
        }

        // Reverse the topological ordering to get the dependency visit ordering.
        result.reverse();
        self.ordered = Rc::new(result);
    }

    /// Initialize every item in the depedency graph using the given provider. The constructed
    /// values will be inserted to this provider and look ups to find the parameters will also be
    /// done on this provider.
    ///
    /// # Events
    ///
    /// This method will automatically trigger the `_post` event on the provider. To learn more
    /// check the documentations around [Eventstore](crate::Eventstore).
    pub fn init_all(mut self, provider: &mut Provider) -> Result<()> {
        self.ensure_topo_order();
        self.capture_events_from_already_constructed(provider);

        for ty in self.ordered.clone().iter() {
            self.construct_internal(*ty, provider)?;
        }

        provider.trigger("_post");

        Ok(())
    }

    /// Initialize the provided value in the provider. This method will call the constructor on only
    /// the relevant subset of the graph which is required for constructing the requested type.
    ///
    /// # Events
    ///
    /// After the initialization every newly registered `_post` event handler is called.
    pub fn init_one<T: 'static>(&mut self, provider: &mut Provider) -> Result<()> {
        self.init_many(provider, vec![Ty::of::<T>()])
    }

    /// Like [`init_one`](Self::init_one) but performs many initializations at one step.
    ///
    /// # Events
    ///
    /// After the initialization every newly registered `_post` event handler is called. This method
    /// also resolves all of the dependencies needed in order to make the call to `_post` before
    /// triggering the event.
    ///
    /// In other word all of the constructors are called before triggering `_post`.
    pub fn init_many(&mut self, provider: &mut Provider, types: Vec<Ty>) -> Result<()> {
        self.ensure_topo_order();
        self.capture_events_from_already_constructed(provider);

        let mut queue = types;

        loop {
            let mut should_init = HashSet::new();

            while let Some(ty) = queue.pop() {
                // If we have already visited this node or it's already present in the provider
                // there is no point in collecting its dependencies.
                if should_init.contains(&ty) || provider.contains_ty(&ty) {
                    continue;
                }

                should_init.insert(ty);

                if let Some(deps) = self.graph.get(&ty) {
                    queue.extend(deps.iter().map(|(_, ty)| ty).copied());
                }
            }

            if should_init.is_empty() {
                break;
            }

            for ty in self.ordered.clone().iter() {
                if should_init.remove(ty) {
                    self.construct_internal(*ty, provider)?;
                }
            }

            if !should_init.is_empty() {
                let ty = should_init.into_iter().next().unwrap();
                panic!("Could not construct a value for '{ty:?}'");
            }

            // Here we ensure that we also have all of the dependencies
            let events = provider.get::<Eventstore>();
            let deps = events.get_dependencies("_post");
            queue.extend(deps.into_iter().map(|(_, ty)| ty));
        }

        provider.trigger("_post");

        Ok(())
    }

    /// Trigger the event with the provided name using this dependency graph along with the provided
    /// provider.
    ///
    /// Unlike [Provider::trigger] this method actually cares about the dependencies required by the
    /// registred event handlers and tries to initialize them before triggering the event.
    ///
    /// This is particularly useful when you wish to initialize a subset of a system and avoid using
    /// `init_all`.
    pub fn trigger(&mut self, provider: &mut Provider, event: &'static str) -> Result<()> {
        let deps = {
            let events = provider.get::<Eventstore>();
            events.get_dependencies(event)
        };

        self.init_many(provider, Vec::from_iter(deps.into_iter().map(|(_, ty)| ty)))?;
        provider.trigger(event);

        Ok(())
    }

    fn construct_internal(&mut self, ty: Ty, provider: &mut Provider) -> Result<()> {
        if provider.contains_ty(&ty) {
            return Ok(());
        }

        let constructor = self
            .constructors
            .remove(&ty)
            .unwrap_or_else(|| panic!("Constructor for type '{}' not provided.", ty.name()));

        let name = constructor.name();
        let rt_name = constructor.ty().name();
        let maybe_events = constructor.events();
        let value = constructor.call(provider);

        let value = value.with_context(|| {
            format!("Error while calling the constructor:\n\t'{name} -> {rt_name}'")
        })?;

        provider.insert_raw(ty, value);

        if let Some(events) = maybe_events {
            let mut event_store = provider.get_mut::<Eventstore>();
            event_store.extend(events);
        }

        Ok(())
    }
}
