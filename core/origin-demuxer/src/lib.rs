mod config;
mod demuxer;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use affair::Socket;
use demuxer::Demuxer;
use derive_more::IsVariant;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ConfigConsumer,
    OriginFetcherInterface,
    OriginProviderInterface,
    OriginProviderSocket,
    WithStartAndShutdown,
};
use lightning_origin_http::HttpOriginFetcher;
use lightning_origin_ipfs::IPFSOrigin;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

pub use crate::config::Config;

#[derive(IsVariant)]
enum Status<C: Collection> {
    Running {
        handle: JoinHandle<Demuxer<C>>,
        shutdown: Arc<Notify>,
    },
    NotRunning {
        demuxer: Demuxer<C>,
    },
}

pub struct OriginDemuxer<C: Collection> {
    status: Mutex<Option<Status<C>>>,
    socket: OriginProviderSocket,
}

impl<C: Collection> ConfigConsumer for OriginDemuxer<C> {
    const KEY: &'static str = "origin-demuxer";
    type Config = Config;
}

impl<C: Collection> WithStartAndShutdown for OriginDemuxer<C> {
    fn is_running(&self) -> bool {
        self.status.blocking_lock().as_ref().unwrap().is_running()
    }

    async fn start(&self) {
        let mut guard = self.status.lock().await;
        let status = guard.take().unwrap();
        let next_status = if let Status::NotRunning { demuxer } = status {
            let (handle, shutdown) = demuxer.spawn();
            Status::Running { handle, shutdown }
        } else {
            status
        };
        *guard = Some(next_status);
    }

    async fn shutdown(&self) {
        let mut guard = self.status.lock().await;
        let status = guard.take().unwrap();
        let next_status = if let Status::Running { handle, shutdown } = status {
            shutdown.notify_one();
            let demuxer = match handle.await {
                Ok(demuxer) => demuxer,
                Err(e) => {
                    std::panic::resume_unwind(e.into_panic());
                },
            };
            Status::NotRunning { demuxer }
        } else {
            status
        };
        *guard = Some(next_status);
    }
}

impl<C: Collection> OriginProviderInterface<C> for OriginDemuxer<C> {
    fn init(config: Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        let (socket, rx) = Socket::raw_bounded(2048);

        let mut demuxer = Demuxer::new(rx);

        demuxer.http_origin(HttpOriginFetcher::<C>::init(
            config.http,
            blockstore.clone(),
        )?);

        demuxer.ipfs_origin(IPFSOrigin::<C>::init(config.ipfs, blockstore.clone())?);

        Ok(Self {
            status: Mutex::new(Some(Status::NotRunning { demuxer })),
            socket,
        })
    }

    fn get_socket(&self) -> OriginProviderSocket {
        self.socket.clone()
    }
}
