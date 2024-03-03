use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;
use std::rc::Rc;

use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};

use crate::method::{DynMethod, Method, ToInfallible, ToResultObject, Value};
use crate::object::Object;
use crate::ty::Ty;
use crate::{Eventstore, Registry};

#[derive(Default)]
pub struct DependencyGraph {
    touched: bool,
    constructors: HashMap<Ty, DynMethod>,
    graph: IndexMap<Ty, IndexSet<Ty>>,
    ordered: Rc<Vec<Ty>>,
}

/// The dependency graph should be used to create a project model. This can be done by providing
/// a set of constructors for different values.
impl DependencyGraph {
    /// Create a new and empty [`DependencyGraph`].
    pub fn new() -> Self {
        Self::default()
    }

    pub fn viz(&self) -> String {
        let mut result = String::with_capacity(4096);
        let mut names = HashMap::new();

        for (ty, method) in &self.constructors {
            let name = method.display_name();
            names.insert(*ty, name);
            writeln!(result, "{name}").unwrap();
        }

        for (ty, dependencies) in &self.graph {
            let ty_name = names.get(ty).copied().unwrap_or(ty.name());

            for dep in dependencies {
                let dep_name = names.get(dep).copied().unwrap_or(dep.name());
                writeln!(result, "{} -> {}", ty_name, dep_name).unwrap();
            }
        }

        result
    }

    /// Expand the current dependency graph from another dependency graph.
    ///
    /// # Panics
    ///
    /// If any of the items in `other` is already present in this graph.
    pub fn expand(mut self, other: DependencyGraph) -> Self {
        self.touched = true;

        for (tid, method) in other.constructors {
            self.insert(tid, method);
        }

        self.graph.extend(other.graph);

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
    /// let mut registry = Registry::default();
    /// graph.init_all(&mut registry).unwrap();
    /// assert_eq!(&*registry.get::<Value>().0, "Fleek");
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
    /// let mut registry = Registry::default();
    /// graph.init_all(&mut registry).unwrap();
    /// assert_eq!(&*registry.get::<String>(), "Hello!");
    /// ```
    pub fn with_value<T>(self, value: T) -> Self
    where
        T: 'static,
    {
        self.with(Value(Ok(value)))
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
    /// let mut registry = Registry::default();
    /// graph.init_all(&mut registry).unwrap();
    /// assert_eq!(&*registry.get::<String>(), "Fleek");
    /// ```
    pub fn with_infallible<F, T, P>(self, f: F) -> Self
    where
        F: Method<T, P>,
        T: 'static,
    {
        self.with(ToInfallible(f))
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
    /// let mut registry = Registry::default();
    /// assert!(graph.init_all(&mut registry).is_err());
    ///
    /// let graph = DependencyGraph::new().with(|| Ok(String::from("Fleek")));
    /// let mut registry = Registry::default();
    /// graph.init_all(&mut registry).unwrap();
    /// assert_eq!(&*registry.get::<String>(), "Fleek");
    /// ```
    pub fn with<F, T, P>(mut self, f: F) -> Self
    where
        F: Method<Result<T>, P>,
        T: 'static,
    {
        self.touched = true;
        let f = ToResultObject::<F, T, P>::new(f);
        let deps = f.dependencies();
        let tid = Ty::of::<T>();
        self.insert(tid, DynMethod::new(f));
        self.graph.insert(tid, deps.into_iter().collect());
        self
    }

    /// Internal method to insert a constructor method to this graph.
    fn insert(&mut self, tid: Ty, method: DynMethod) {
        debug_assert_eq!(method.ty(), Ty::of::<Result<Object>>());
        if let Some(old) = self.constructors.get(&tid) {
            panic!(
                "A constructor for type '{}' is already present.\n\told='{}'\n\tnew='{}'",
                method.ty().name(),
                old.name(),
                method.name()
            );
        }
        self.constructors.insert(tid, method);
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

            for tag in connections {
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
                for v in values {
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

    /// Initialize every item in the depedency graph using the provided registry. The constructed
    /// values will be inserted to this registry and look ups to find the parameters will also be
    /// done on this registry.
    ///
    /// # Events
    ///
    /// This method will automatically trigger the `_post` event on the registry. To learn more
    /// check the documentations around [Eventstore](crate::Eventstore).
    pub fn init_all(mut self, registry: &mut Registry) -> Result<()> {
        self.ensure_topo_order();

        for ty in self.ordered.clone().iter() {
            self.construct_internal(*ty, registry)?;
        }

        registry.trigger("_post");

        Ok(())
    }

    /// Initialize the provided value in the registry. This method will call the constructor on only
    /// the relevant subset of the graph which is required for constructing the requested type.
    ///
    /// # Events
    ///
    /// After the initialization every newly registered `_post` event handler is called.
    pub fn init_one<T: 'static>(&mut self, registry: &mut Registry) -> Result<()> {
        self.init_many(registry, vec![Ty::of::<T>()])
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
    pub fn init_many(&mut self, registry: &mut Registry, types: Vec<Ty>) -> Result<()> {
        self.ensure_topo_order();

        let mut queue = types;

        loop {
            let mut should_init = HashSet::new();

            while let Some(ty) = queue.pop() {
                // If we have already visited this node or it's already present in the registry
                // there is no point in collecting its dependencies.
                if should_init.contains(&ty) || registry.contains_type_id(&ty) {
                    continue;
                }

                should_init.insert(ty);

                if let Some(deps) = self.graph.get(&ty) {
                    queue.extend(deps.iter().copied());
                }
            }

            if should_init.is_empty() {
                break;
            }

            for ty in self.ordered.clone().iter() {
                if should_init.contains(ty) {
                    self.construct_internal(*ty, registry)?;
                }
            }

            // Here we ensure that we also have all of the dependencies
            let events = registry.get::<Eventstore>();
            let deps = events.get_dependencies("_post");
            queue.extend(deps);
        }

        registry.trigger("_post");

        Ok(())
    }

    /// Trigger the event with the provided name using this dependency graph along with the provided
    /// registry.
    ///
    /// Unlike [`Registry::trigger`](crate::Registry::trigger) this method actually cares about the
    /// dependencies required by the registred event handlers and tries to initialize them before
    /// triggering the event.
    ///
    /// This is particularly useful when you wish to initialize a subset of a system and avoid using
    /// `init_all`.
    pub fn trigger(&mut self, registry: &mut Registry, event: &'static str) -> Result<()> {
        let deps = {
            let events = registry.get::<Eventstore>();
            events.get_dependencies(event)
        };

        self.init_many(registry, Vec::from_iter(deps))?;
        registry.trigger(event);

        Ok(())
    }

    fn construct_internal(&mut self, ty: Ty, registry: &mut Registry) -> Result<()> {
        if registry.contains_type_id(&ty) {
            return Ok(());
        }

        let constructor = self
            .constructors
            .remove(&ty)
            .unwrap_or_else(|| panic!("Constructor for type not provided. tid={ty:?}"));

        let name = constructor.name();
        let rt_name = constructor.ty().name();
        let value = constructor.call(registry);

        let value = value
            .downcast::<Result<Object>>()
            .unwrap()
            .with_context(|| {
                format!("Error while calling the constructor:\n\t'{name} -> {rt_name}'")
            })?;

        registry.insert_raw(value);
        Ok(())
    }
}
