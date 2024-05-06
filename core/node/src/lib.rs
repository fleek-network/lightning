use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use lightning_interfaces::prelude::*;

/// A single [Node] instance that has ownership over its tokio runtime.
pub struct ContainedNode<C: Collection> {
    /// The dependency injection data provider which can contain most of the items that make up
    /// a node.
    provider: fdi::Provider,

    /// A handle to the work thread.
    handle: JoinHandle<()>,

    collection: PhantomData<C>,
}

impl<C: Collection> ContainedNode<C> {
    pub fn new(provider: fdi::Provider, index: usize) -> Self {
        let worker_id = AtomicUsize::new(0);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name_fn(move || {
                let id = worker_id.fetch_add(1, Ordering::SeqCst);
                format!("NODE-{index}#{id}")
            })
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime for node container.");

        let handle = std::thread::Builder::new()
            .name(format!("NODE-{index}#MAIN"))
            .spawn(move || {
                runtime.block_on(async move {
                    //
                });
            })
            .expect("Failed to spawn E2E thread");

        Self {
            provider,
            handle,
            collection: PhantomData,
        }
    }

    /// Returns a reference to the data provider.
    pub fn provider(&self) -> &fdi::Provider {
        &self.provider
    }

    pub fn shutdown(self) {
        self.handle.join().unwrap()
    }
}

impl<C: Collection> Default for ContainedNode<C> {
    fn default() -> Self {
        Self::new(fdi::Provider::default(), 0)
    }
}
