use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use affair::{Socket, Task};
use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{ArchiveRequest, ArchiveResponse, IndexRequest};
use lightning_interfaces::{
    ApplicationInterface,
    ArchiveInterface,
    ArchiveSocket,
    ConfigConsumer,
    IndexSocket,
    WithStartAndShutdown,
};
use log::error;
use rocksdb::{Options, DB};
use tokio::sync::{mpsc, Notify};

use crate::config::Config;

const BLKHASH_TO_BLKNUM: &str = "blkhash_to_blknum";
const BLKNUM_TO_BLK: &str = "blknum_to_blk";
const TXHASH_TO_TXRCT: &str = "txhash_to_txrct";

type ArchiveTask = Task<ArchiveRequest, ArchiveResponse>;
type IndexTask = Task<IndexRequest, ()>;

pub struct Archive<C: Collection> {
    inner: Option<Arc<ArchiveInner<C>>>,
    /// This socket can be given out to other proccess to query the data that has been archived.
    /// Will be None if this node is not currently in archive mode
    archive_socket: Mutex<Option<ArchiveSocket>>,
    /// This socket can be given out to other process to send things that should be archived,
    /// realisticlly only consensus should have this. Will return None if the node is not currently
    /// in archive mode
    index_socket: Mutex<Option<IndexSocket>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Option<Arc<Notify>>,
    _marker: PhantomData<C>,
}

impl<C: Collection> ArchiveInterface<C> for Archive<C> {
    fn init(
        config: Self::Config,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        if config.is_archive {
            let db_path = config
                .store_path
                .expect("Must specify db path if archive service is enabled");
            let shutdown_notify = Arc::new(Notify::new());
            let (archive_socket, archive_rx) = Socket::raw_bounded(2048);
            let (index_socket, index_rx) = Socket::raw_bounded(2048);

            let mut db_options = Options::default();
            db_options.create_if_missing(true);
            db_options.create_missing_column_families(true);

            let cf = vec![BLKHASH_TO_BLKNUM, BLKNUM_TO_BLK, TXHASH_TO_TXRCT];
            let db = Arc::new(
                DB::open_cf(&db_options, db_path, cf).expect("Failed to create archive db"),
            );

            let inner = ArchiveInner::<C>::new(archive_rx, index_rx, db, shutdown_notify.clone());
            Ok(Self {
                archive_socket: Mutex::new(Some(archive_socket)),
                index_socket: Mutex::new(Some(index_socket)),
                inner: Some(Arc::new(inner)),
                is_running: Arc::new(AtomicBool::new(false)),
                shutdown_notify: Some(shutdown_notify),
                _marker: PhantomData,
            })
        } else {
            Ok(Self {
                archive_socket: Mutex::new(None),
                index_socket: Mutex::new(None),
                inner: None,
                is_running: Arc::new(AtomicBool::new(false)),
                shutdown_notify: None,
                _marker: PhantomData,
            })
        }
    }

    fn archive_socket(&self) -> Option<ArchiveSocket> {
        self.archive_socket.lock().unwrap().clone()
    }

    fn index_socket(&self) -> Option<IndexSocket> {
        self.index_socket.lock().unwrap().clone()
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Archive<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        if !self.is_running() {
            if let Some(inner) = self.inner.clone() {
                let is_running = self.is_running.clone();
                tokio::spawn(async move {
                    inner.start().await;
                    is_running.store(false, Ordering::Relaxed);
                });
                self.is_running.store(true, Ordering::Relaxed);
            }
        } else {
            error!("Can not start archive because it is already running");
        }
    }

    async fn shutdown(&self) {
        if let Some(shutdown_notify) = self.shutdown_notify.clone() {
            shutdown_notify.notify_one();
        }
    }
}

struct ArchiveInner<C: Collection> {
    archive_rx: Arc<Mutex<Option<mpsc::Receiver<ArchiveTask>>>>,
    index_rx: Arc<Mutex<Option<mpsc::Receiver<IndexTask>>>>,
    db: Arc<DB>,
    shutdown_notify: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection> ArchiveInner<C> {
    fn new(
        archive_rx: mpsc::Receiver<Task<ArchiveRequest, ArchiveResponse>>,
        index_rx: mpsc::Receiver<Task<IndexRequest, ()>>,
        db: Arc<DB>,
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            archive_rx: Arc::new(Mutex::new(Some(archive_rx))),
            index_rx: Arc::new(Mutex::new(Some(index_rx))),
            db,
            shutdown_notify,
            _marker: PhantomData,
        }
    }

    async fn start(&self) {
        let mut archive_rx = self.archive_rx.lock().unwrap().take().unwrap();
        let mut index_rx = self.index_rx.lock().unwrap().take().unwrap();
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                Some(archive_task) = archive_rx.recv() => {
                    self.archive_request(archive_task).await;
                }
                Some(index_task) = index_rx.recv() => {
                    self.index_request(index_task).await;

                }
            }
        }
        *self.archive_rx.lock().unwrap() = Some(archive_rx);
        *self.index_rx.lock().unwrap() = Some(index_rx);
    }

    async fn archive_request(&self, archive_task: ArchiveTask) {}

    async fn index_request(&self, index_task: IndexTask) {}
}

impl<C: Collection> ConfigConsumer for Archive<C> {
    const KEY: &'static str = "archive";

    type Config = Config;
}
