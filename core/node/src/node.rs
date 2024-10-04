use std::marker::PhantomData;
use std::time::Duration;

use anyhow::Result;
use fdi::Provider;
use lightning_interfaces::prelude::*;
use lightning_interfaces::ShutdownController;
use tokio::time::sleep;

pub struct Node<C: NodeComponents> {
    pub provider: Provider,
    pub shutdown: Option<ShutdownController>,
    _p: PhantomData<C>,
}

impl<C: NodeComponents> Node<C> {
    #[inline(always)]
    pub fn init(config: C::ConfigProviderInterface) -> Result<Self> {
        let provider = Provider::default();
        provider.insert(config);
        Self::init_with_provider(provider)
    }

    pub fn init_with_provider(mut provider: Provider) -> Result<Self> {
        let mut exec = provider.get_mut::<fdi::Executor>();
        exec.set_spawn_cb(|fut, _name| {
            tokio::spawn(fut);
        });

        let graph = C::build_graph();

        // let vis = graph.viz("Lightning Dependency Graph");
        // println!("{vis}");

        graph.init_all(&mut provider)?;

        let shutdown = provider.take();

        Ok(Self {
            provider,
            shutdown: Some(shutdown),
            _p: PhantomData,
        })
    }

    pub async fn start(&self) {
        self.provider.trigger("start");
    }

    pub fn shutdown_waiter(&self) -> Option<ShutdownWaiter> {
        self.shutdown.as_ref().map(|s| s.waiter())
    }

    /// Shutdown the node
    pub async fn shutdown(&mut self) {
        let mut shutdown = self
            .shutdown
            .take()
            .expect("cannot call shutdown more than once");

        tracing::trace!("Shutting node down.");
        shutdown.trigger_shutdown();

        for i in 0.. {
            tokio::select! {
                biased;
                _ = shutdown.wait_for_completion() => {
                    return;
                },
                _ = sleep(Duration::from_secs(5)) => {
                    match i {
                        0 => {
                            tracing::trace!("Still shutting down...");
                            continue;
                        },
                        1 => {
                            tracing::warn!("Still shutting down...");
                            continue;
                        },
                        _ => {
                            tracing::error!("Shutdown taking too long")
                        }
                    }
                }
            }

            let Some(iter) = shutdown.pending_backtraces() else {
                continue;
            };

            eprintln!("Printing pending backtraces:");
            for (i, trace) in iter.enumerate() {
                eprintln!("Pending task backtrace #{i}:\n{trace:#?}");
            }
        }
    }
}
