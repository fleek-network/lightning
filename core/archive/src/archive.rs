use std::marker::PhantomData;
use std::sync::Mutex;

use affair::{Executor, TokioSpawn};
use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ArchiveInterface,
    ArchiveSocket,
    ConfigConsumer,
    IndexSocket,
    WithStartAndShutdown,
};

use crate::config::Config;
use crate::indexer::IndexWorker;
use crate::query::QueryWorker;

pub struct Archive<C: Collection> {
    /// This socket can be given out to other proccess to query the data that has been archived.
    /// Will be None if this node is not currently in archive mode
    archive_socket: Mutex<Option<ArchiveSocket>>,
    /// This socket can be given out to other process to send things that should be archived,
    /// realisticlly only consensus should have this. Will return None if the node is not currently
    /// in archive mode
    index_socket: Mutex<Option<IndexSocket>>,
    /// The worker that can be turned into a socket, kept on the struct so we can support the
    /// init/start flow and be restartable
    archive_worker: Option<QueryWorker>,
    /// The worker that can be turned into a socket, kept on the struct so we can support the
    /// init/start flow and be restartable
    index_worker: Option<IndexWorker>,
    collection: PhantomData<C>,
}

impl<C: Collection> ArchiveInterface<C> for Archive<C> {
    fn init(
        config: Self::Config,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        if config.is_archive {
            Ok(Self {
                archive_socket: Mutex::new(None),
                index_socket: Mutex::new(None),
                archive_worker: Some(QueryWorker::new()),
                index_worker: Some(IndexWorker::new()),
                collection: PhantomData,
            })
        } else {
            Ok(Self {
                archive_socket: Mutex::new(None),
                index_socket: Mutex::new(None),
                archive_worker: None,
                index_worker: None,
                collection: PhantomData,
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
        self.archive_socket.lock().unwrap().is_some()
    }

    async fn start(&self) {
        if self.is_running() {
            return;
        }

        if let (Some(archive_worker), Some(index_worker)) =
            (self.archive_worker.clone(), self.index_worker.clone())
        {
            // This node is in archive mode we should start our workers
            *self.archive_socket.lock().unwrap() = Some(TokioSpawn::spawn_async(archive_worker));

            *self.index_socket.lock().unwrap() = Some(TokioSpawn::spawn_async(index_worker));
        }
    }

    async fn shutdown(&self) {
        *self.archive_socket.lock().unwrap() = None;
        *self.index_socket.lock().unwrap() = None;
    }
}

impl<C: Collection> ConfigConsumer for Archive<C> {
    const KEY: &'static str = "archive";

    type Config = Config;
}
