use std::any::{type_name, Any, TypeId};
use std::collections::{HashMap, VecDeque};

use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};

use crate::method::{DynMethod, Method, ToInfallible, ToResultBoxAny};
use crate::registry::MutRegistry;

#[derive(Default)]
pub struct DependencyGraph {
    touched: bool,
    constructors: HashMap<TypeId, DynMethod>,
    graph: IndexMap<TypeId, IndexSet<TypeId>>,
    ordered: Vec<TypeId>,
}

impl DependencyGraph {
    pub fn with_infallible<F, T, P>(self, f: F) -> Self
    where
        F: Method<T, P>,
        T: 'static,
    {
        self.with(ToInfallible(f))
    }

    pub fn with<F, T, P>(mut self, f: F) -> Self
    where
        F: Method<Result<T>, P>,
        T: 'static,
    {
        let f = ToResultBoxAny::<F, T, P>::new(f);
        self.touched = true;
        let new_name = f.name();
        let deps = f.dependencies();
        let tid = TypeId::of::<T>();
        if let Some(old) = self.constructors.insert(tid, DynMethod::new(f)) {
            panic!(
                "A constructor for type '{}' is already present.\n\told='{}'\n\tnew='{}'",
                type_name::<T>(),
                old.name(),
                new_name
            );
        }
        self.graph.insert(tid, deps.into_iter().collect());
        self
    }

    fn ensure_topo_order(&mut self) {
        if !self.touched {
            return;
        }

        self.touched = true;
        self.ordered.clear();

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
            self.ordered.push(u);

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
        self.ordered.reverse();
    }

    pub fn init_all(&mut self, registry: &mut MutRegistry) -> Result<()> {
        self.ensure_topo_order();

        for tid in &self.ordered {
            if registry.contains_type_id(tid) {
                continue;
            }

            let constructor = self.constructors.remove(tid).expect(
                "If the constructor is not present it is expected to already be in the registry.",
            );

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
        }

        registry.trigger("_post");

        Ok(())
    }

    pub fn init_only<T: 'static>(&mut self, _registry: &mut MutRegistry) {
        self.ensure_topo_order();
    }
}

#[test]
fn xxx() {
    use crate::event::Eventstore;

    #[derive(Debug)]
    struct Thing1(u32);
    #[derive(Debug)]
    struct Thing2(u32);

    fn make_thing2(events: &mut Eventstore) -> Result<Thing2> {
        events.on("_post", |this: &mut Thing2| {
            dbg!(this);
        });

        Ok(Thing2(12))
    }

    let mut graph = DependencyGraph::default()
        .with_infallible(|| Thing1(12))
        .with(make_thing2);

    let mut registry = MutRegistry::default();
    graph.init_all(&mut registry).unwrap();

    let thing1 = &*registry.get::<Thing1>();
    println!("{thing1:?}");
}
