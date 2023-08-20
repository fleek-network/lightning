use std::marker::PhantomData;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use lightning_interfaces::infu_collection::{Collection, Node};
use tokio::sync::Notify;

use crate::containerized_node::RuntimeType;

#[derive(Debug)]
pub struct Container<C: Collection> {
    join_handle: Option<JoinHandle<()>>,
    shutdown_notify: Option<Arc<Notify>>,
    collection: PhantomData<C>,
}

impl<C: Collection> Drop for Container<C> {
    fn drop(&mut self) {
        let handle = self.join_handle.take().unwrap();
        let shutdown_notify = self.shutdown_notify.take().unwrap();
        shutdown_notify.notify_one();
        handle.join().unwrap();
    }
}

impl<C: Collection> Container<C> {
    pub async fn spawn(config: C::ConfigProviderInterface, runtime_type: RuntimeType) -> Self {
        let shutdown_notify = Arc::new(Notify::new());
        let shutdown_notify_rx = shutdown_notify.clone();

        let handle = thread::spawn(move || {
            let mut builder = match runtime_type {
                RuntimeType::SingleThreaded => tokio::runtime::Builder::new_current_thread(),
                RuntimeType::MultiThreaded => tokio::runtime::Builder::new_multi_thread(),
            };

            let runtime = builder
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime for node container.");

            runtime.block_on(async move {
                let node = Node::<C>::init(config).unwrap();
                node.start().await;

                shutdown_notify_rx.notified().await;
                node.shutdown().await;
            });
        });

        Self {
            join_handle: Some(handle),
            shutdown_notify: Some(shutdown_notify),
            collection: PhantomData,
        }
    }
}
