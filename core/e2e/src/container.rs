use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use lightning_interfaces::{Node, WithStartAndShutdown};
use lightning_node::{config::TomlConfigProvider, node::FinalTypes};
use tokio::sync::Notify;

use crate::containerized_node::RuntimeType;

#[derive(Debug)]
pub struct Container {
    join_handle: Option<JoinHandle<()>>,
    shutdown_notify: Option<Arc<Notify>>,
}

impl Drop for Container {
    fn drop(&mut self) {
        let handle = self.join_handle.take().unwrap();
        let shutdown_notify = self.shutdown_notify.take().unwrap();
        shutdown_notify.notify_one();
        handle.join().unwrap();
    }
}

impl Container {
    pub async fn spawn(config: Arc<TomlConfigProvider>, runtime_type: RuntimeType) -> Self {
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
                let node = Node::<FinalTypes>::init(config).await.unwrap();
                node.start().await;

                shutdown_notify_rx.notified().await;
                node.shutdown().await;
            });
        });

        Self {
            join_handle: Some(handle),
            shutdown_notify: Some(shutdown_notify),
        }
    }
}
