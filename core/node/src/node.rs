use std::marker::PhantomData;
use std::time::Duration;

use anyhow::Result;
use fdi::Provider;
use lightning_checkpointer::Checkpointer;
use lightning_consensus::Consensus;
use lightning_interfaces::prelude::*;
use lightning_interfaces::ShutdownController;
use lightning_pool::PoolProvider;
use lightning_rpc::Rpc;
use tokio::time::sleep;

use crate::NodeError;

pub struct Node<C: NodeComponents> {
    pub provider: Provider,
    pub shutdown: Option<ShutdownController>,
    _components: PhantomData<C>,
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

        graph.init_all(&mut provider)?;

        let shutdown = provider.take();

        Ok(Self {
            provider,
            shutdown: Some(shutdown),
            _components: PhantomData,
        })
    }

    pub async fn start(&self) {
        self.provider.trigger("start");
    }

    pub async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<(), NodeError> {
        let wait_fut = async {
            self.provider
                .get::<PoolProvider<C>>()
                .wait_for_ready()
                .await;
            self.provider.get::<Rpc<C>>().wait_for_ready().await;
            self.provider
                .get::<Checkpointer<C>>()
                .wait_for_ready()
                .await;
            self.provider.get::<Consensus<C>>().wait_for_ready().await;
        };

        match timeout {
            Some(timeout) => tokio::time::timeout(timeout, wait_fut)
                .await
                .map_err(From::from),
            None => {
                wait_fut.await;
                Ok(())
            },
        }
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
