use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use lightning_interfaces::prelude::*;
use tokio::sync::Notify;

use crate::containerized_node::RuntimeType;

pub struct Container<C: Collection> {
    join_handle: Option<JoinHandle<()>>,
    shutdown_notify: Option<Arc<Notify>>,
    syncronizer: Option<fdi::Ref<C::SyncronizerInterface>>,
    blockstore: Option<C::BlockstoreInterface>,
}

impl<C: Collection> Drop for Container<C> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl<C: Collection> Container<C> {
    pub async fn spawn(
        index: usize,
        config: C::ConfigProviderInterface,
        runtime_type: RuntimeType,
    ) -> Self {
        let shutdown_notify = Arc::new(Notify::new());
        let shutdown_notify_rx = shutdown_notify.clone();

        let (tx, rx) = tokio::sync::oneshot::channel();
        let handle = std::thread::Builder::new()
            .name(format!("NODE-{index}#MAIN"))
            .spawn(move || {
                let mut builder = match runtime_type {
                    RuntimeType::SingleThreaded => tokio::runtime::Builder::new_current_thread(),
                    RuntimeType::MultiThreaded => tokio::runtime::Builder::new_multi_thread(),
                };

                let runtime = builder
                    .thread_name_fn(move || {
                        static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                        let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                        format!("NODE-{index}#{id}")
                    })
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime for node container.");

                runtime.block_on(async move {
                    let mut node = Node::<C>::init(config).unwrap();
                    node.start().await;

                    tokio::time::sleep(Duration::from_millis(10)).await;

                    let syncronizer = node
                        .provider
                        .get::<<C as Collection>::SyncronizerInterface>();
                    let blockstore = node
                        .provider
                        .get::<<C as Collection>::BlockstoreInterface>()
                        .clone();

                    tx.send((syncronizer, blockstore)).ok();

                    shutdown_notify_rx.notified().await;
                    node.shutdown().await;
                });
            })
            .expect("Failed to spawn E2E thread");

        let (syncronizer, blockstore) = rx.await.expect("Failed to receive");

        Self {
            join_handle: Some(handle),
            shutdown_notify: Some(shutdown_notify),
            syncronizer: Some(syncronizer),
            blockstore: Some(blockstore),
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            let shutdown_notify = self.shutdown_notify.take().unwrap();
            shutdown_notify.notify_one();
            handle.join().expect("Failed to shutdown container.");
        }
    }

    pub fn take_ckpt_rx(&mut self) -> Option<fdi::Ref<C::SyncronizerInterface>> {
        self.syncronizer.take()
    }

    pub fn take_blockstore(&mut self) -> Option<C::BlockstoreInterface> {
        self.blockstore.take()
    }
}
