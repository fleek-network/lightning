use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use infusion::tag;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::types::Blake3Hash;
use lightning_interfaces::{BlockStoreInterface, DhtInterface, DhtSocket, SyncronizerInterface};
use tokio::sync::{oneshot, Notify};

use crate::containerized_node::RuntimeType;

pub struct Container<C: Collection> {
    join_handle: Option<JoinHandle<()>>,
    shutdown_notify: Option<Arc<Notify>>,
    ckpt_rx: Option<oneshot::Receiver<Blake3Hash>>,
    dht_socket: Option<DhtSocket>,
    blockstore: Option<C::BlockStoreInterface>,
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
        let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();

        let (tx, rx) = std::sync::mpsc::channel();
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
                    let node = Node::<C>::init(config).unwrap();
                    node.start().await;
                    let ckpt_rx = node
                        .container
                        .get::<<C as Collection>::SyncronizerInterface>(tag!(
                            C::SyncronizerInterface
                        ))
                        .checkpoint_socket();
                    let dht_socket = node
                        .container
                        .get::<<C as Collection>::DhtInterface>(tag!(C::DhtInterface))
                        .get_socket();
                    let blockstore = node
                        .container
                        .get::<<C as Collection>::BlockStoreInterface>(tag!(C::BlockStoreInterface))
                        .clone();

                    tx.send((ckpt_rx, dht_socket, blockstore))
                        .expect("Failed to send");

                    let _ = started_tx.send(());

                    shutdown_notify_rx.notified().await;
                    node.shutdown().await;
                });
            })
            .expect("Failed to spawn E2E thread");

        let (ckpt_rx, dht_socket, blockstore) = rx.recv().expect("Failed to receive");
        started_rx.await.expect("Failed to start the node.");

        Self {
            join_handle: Some(handle),
            shutdown_notify: Some(shutdown_notify),
            ckpt_rx: Some(ckpt_rx),
            dht_socket: Some(dht_socket),
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

    pub fn take_ckpt_rx(&mut self) -> Option<oneshot::Receiver<Blake3Hash>> {
        self.ckpt_rx.take()
    }

    pub fn take_dht_socket(&mut self) -> Option<DhtSocket> {
        self.dht_socket.take()
    }

    pub fn take_blockstore(&mut self) -> Option<C::BlockStoreInterface> {
        self.blockstore.take()
    }
}
