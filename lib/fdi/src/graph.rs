use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};

use crate::method::{DynMethod, Method, ToInfallible, ToResultBoxAny, Value};
use crate::registry::MutRegistry;
use crate::Eventstore;

#[derive(Default)]
pub struct DependencyGraph {
    touched: bool,
    constructors: HashMap<TypeId, DynMethod>,
    graph: IndexMap<TypeId, IndexSet<TypeId>>,
    ordered: Rc<Vec<TypeId>>,
}

/// The dependency graph should be used to create a project model. This can be done by providing
/// a set of constructors for different values.
impl DependencyGraph {
    /// Create a new and empty [`DependencyGraph`].
    pub fn new() -> Self {
        Self::default()
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
    /// let mut registry = MutRegistry::default();
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
    /// let mut registry = MutRegistry::default();
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
    /// let mut registry = MutRegistry::default();
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
    /// let mut registry = MutRegistry::default();
    /// assert!(graph.init_all(&mut registry).is_err());
    ///
    /// let graph = DependencyGraph::new().with(|| Ok(String::from("Fleek")));
    /// let mut registry = MutRegistry::default();
    /// graph.init_all(&mut registry).unwrap();
    /// assert_eq!(&*registry.get::<String>(), "Fleek");
    /// ```
    pub fn with<F, T, P>(mut self, f: F) -> Self
    where
        F: Method<Result<T>, P>,
        T: 'static,
    {
        self.touched = true;
        let f = ToResultBoxAny::<F, T, P>::new(f);
        let deps = f.dependencies();
        let tid = TypeId::of::<T>();
        self.insert(tid, DynMethod::new(f));
        self.graph.insert(tid, deps.into_iter().collect());
        self
    }

    /// Internal method to insert a constructor method to this graph.
    fn insert(&mut self, tid: TypeId, method: DynMethod) {
        debug_assert_eq!(method.type_id(), TypeId::of::<Result<Box<dyn Any>>>());
        if let Some(old) = self.constructors.get(&tid) {
            panic!(
                "A constructor for type '{}' is already present.\n\told='{}'\n\tnew='{}'",
                method.return_type_name(),
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
        let mut queue = VecDeque::<TypeId>::with_capacity(len);

        // Map each node to its in-degree.
        let mut in_degree = IndexMap::<TypeId, usize>::with_capacity(len);

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
    pub fn init_all(mut self, registry: &mut MutRegistry) -> Result<()> {
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
    pub fn init_one<T: 'static>(&mut self, registry: &mut MutRegistry) -> Result<()> {
        self.init_many(registry, vec![TypeId::of::<T>()])
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
    pub fn init_many(&mut self, registry: &mut MutRegistry, types: Vec<TypeId>) -> Result<()> {
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
            let registry = registry.as_reader();
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
    pub fn trigger(&mut self, registry: &mut MutRegistry, event: &'static str) -> Result<()> {
        let deps = {
            let registry = registry.as_reader();
            let events = registry.get::<Eventstore>();
            events.get_dependencies(event)
        };

        self.init_many(registry, Vec::from_iter(deps))?;
        registry.trigger(event);

        Ok(())
    }

    fn construct_internal(&mut self, ty: TypeId, registry: &mut MutRegistry) -> Result<()> {
        if registry.contains_type_id(&ty) {
            return Ok(());
        }

        let constructor = self
            .constructors
            .remove(&ty)
            .unwrap_or_else(|| panic!("Constructor for type not provided. tid={ty:?}"));

        let name = constructor.name();
        let rt_name = constructor.return_type_name();

        let value = {
            let reader = registry.as_reader();
            constructor.call(&reader)
        };

        let value = value
            .downcast::<Result<Box<dyn Any>>>()
            .unwrap()
            .with_context(|| {
                format!("Error while calling the constructor:\n\t'{name} -> {rt_name}'")
            })?;

        registry.insert_raw(value);
        Ok(())
    }
}

#[test]
fn xxx() {
    use crate::event::Eventstore;

    #[derive(Debug)]
    struct Thing1(u32);
    #[derive(Debug)]
    struct Thing2(u32);
    struct SubComponentOfThing2;

    impl Thing2 {
        pub fn get_sub(&self) -> SubComponentOfThing2 {
            SubComponentOfThing2
        }
    }

    fn make_thing2(events: &mut Eventstore) -> Result<Thing2> {
        events.on("_post", |this: &mut Thing2| {
            dbg!(this);
        });

        Ok(Thing2(15))
    }

    let graph = DependencyGraph::default()
        .with_infallible(|| Thing1(12))
        .with(make_thing2)
        .with_infallible(Thing2::get_sub);

    let mut registry = MutRegistry::default();
    graph.init_all(&mut registry).unwrap();

    let thing1 = &*registry.get::<Thing1>();
    println!("{thing1:?}");

    let thing2 = &*registry.get::<Thing2>();
    println!("{thing2:?}");
}
