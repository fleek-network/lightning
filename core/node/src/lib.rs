use std::future::Future;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_interfaces::ShutdownController;
use tokio::runtime::{Handle, Runtime};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// A single [Node] instance that has ownership over its tokio runtime.
pub struct ContainedNode<C: NodeComponents> {
    /// The name of this contained node.
    name: String,

    /// The dependency injection data provider which can contain most of the items that make up
    /// a node.
    provider: fdi::MultiThreadedProvider,

    /// The shutdown controller that has its waiter in the provider.
    shutdown: ShutdownController,

    /// A handle to the tokio runtime.
    runtime: Option<Runtime>,

    _components: PhantomData<C>,
}

impl<C: NodeComponents> ContainedNode<C> {
    pub fn new(provider: fdi::MultiThreadedProvider, name: Option<String>) -> Self {
        let name = name.unwrap_or_else(|| "LIGHTNING".into());

        // Create and insert the shutdown controller to the provider.
        let trace_shutdown = std::env::var("TRACE_SHUTDOWN").is_ok();
        let shutdown = ShutdownController::new(trace_shutdown);
        let waiter = shutdown.waiter();
        provider.insert(waiter);

        // Get the trigger permit from the shutdown controller to be passed into each thread.
        //let permit = shutdown.permit();

        // Create the tokio runtime.
        let worker_id = AtomicUsize::new(0);
        let node_name = name.clone();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name_fn(move || {
                let id = worker_id.fetch_add(1, Ordering::SeqCst);
                format!("{node_name}#{id}")
            })
            .on_thread_start(move || {
                //let permit = permit.clone();
                thread_local_panic_hook::update_hook(move |prev, info| {
                    tracing::error!("Uncaught panic detected in worker.");
                    //permit.trigger_shutdown();
                    // bubble up and call the previous panic handler.
                    prev(info);
                });
            })
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime for node container.");

        // Run the `install_ctrlc_handlers` in the context of Tokio.
        let guard = runtime.enter();
        // Will make the shutdown controller listen for ctrl+c.
        shutdown.install_ctrlc_handlers();
        drop(guard);

        Self {
            name,
            provider,
            runtime: Some(runtime),
            shutdown,
            _components: PhantomData,
        }
    }

    /// Start the node and return a handle to the task that started the node. The task returns as
    /// soon as the node starts up.
    pub fn spawn(&self) -> JoinHandle<Result<()>> {
        let provider = self.provider.clone();

        let waiter = self.shutdown.waiter();
        self.runtime.as_ref().unwrap().spawn_blocking(move || {
            let graph = C::build_graph();
            let mut provider = provider.get_local_provider();

            // Set tokio as the spawner of fdi async works.
            provider
                .get_mut::<fdi::Executor>()
                .set_spawn_cb(move |fut, name| {
                    let name = name.expect("Name must be provided");
                    let waiter = waiter.clone();
                    spawn!(fut, &name, crucial(waiter));
                });

            // Init all of the components and dependencies.
            graph.init_all(&mut provider)?;

            // Send the start signal to the node.
            provider.trigger("start");

            Ok(())
        })
    }

    /// Shut down the node and return a future that will be resolved when the node is fully down.
    ///
    /// Unlike other async method this function can trigger the shutdown without it being polled.
    /// In other words you can still trigger the shutdown event by calling this method and never
    /// awaiting the returned future.
    pub fn shutdown(mut self) -> impl Future<Output = ()> {
        let handle =
            Handle::try_current().expect("calling from a non-tokio context not supported yet.");

        // Take state out (because we implement drop).
        let mut shutdown = std::mem::take(&mut self.shutdown);

        // Tell the controller it's time to go down.
        shutdown.trigger_shutdown();

        let task_name = format!("{}::RuntimeDrop", self.name);

        async move {
            for i in 0.. {
                if timeout(Duration::from_secs(3), shutdown.wait_for_completion())
                    .await
                    .is_ok()
                {
                    // shutdown completed.
                    break;
                }

                match i {
                    0 | 1 => {
                        // 3s, 6s
                        tracing::trace!("Still shutting down...");
                        continue;
                    },
                    2 => {
                        // 9s
                        tracing::warn!("Still shutting down...");
                        continue;
                    },
                    _ => {
                        // 12s
                        tracing::error!("Shutdown taking too long..")
                    },
                }

                if i == 9 {
                    // 30s: timeout
                    tracing::error!("Shutdown timed out. Force killing the runtime.");
                    break;
                }

                let Some(iter) = shutdown.pending_backtraces() else {
                    continue;
                };

                for (i, trace) in iter.enumerate() {
                    eprintln!("Pending task backtrace #{i}:\n{trace:#?}");
                }
            }

            let runtime = self.runtime.take().unwrap();
            tokio::task::Builder::new()
                .name(&task_name)
                .spawn_blocking_on(
                    || {
                        drop(runtime);
                    },
                    &handle,
                )
                .unwrap()
                .await
                .expect("Failed to wait for the node runtime to drop.");
        }
    }

    /// Returns a reference to the data provider.
    pub fn provider(&self) -> &fdi::MultiThreadedProvider {
        &self.provider
    }
}

impl<C: NodeComponents> Default for ContainedNode<C> {
    fn default() -> Self {
        Self::new(fdi::MultiThreadedProvider::default(), None)
    }
}

impl<C: NodeComponents> Drop for ContainedNode<C> {
    fn drop(&mut self) {
        // if runtime doesn't exist it means `shutdown` has been called before.
        if let Some(runtime) = self.runtime.take() {
            self.shutdown.trigger_shutdown();

            // If we're running within nested runtime env, dropping the runtime in async
            // context would not be allowed by tokio. so we have to use spawn blocking
            // here.
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn_blocking(move || {
                    drop(runtime);
                });
            }
        }
    }
}
