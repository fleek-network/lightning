mod config;
mod muxer;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use affair::Socket;
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
use muxer::Muxer;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use crate::config::Config;

#[derive(IsVariant)]
enum Status<C: Collection> {
    Running {
        handle: JoinHandle<Muxer<C>>,
        shutdown: Arc<Notify>,
    },
    NotRunning {
        muxer: Muxer<C>,
    },
}

pub struct OriginMuxer<C: Collection> {
    status: Mutex<Option<Status<C>>>,
    socket: OriginProviderSocket,
}

impl<C: Collection> ConfigConsumer for OriginMuxer<C> {
    const KEY: &'static str = "origin-muxer";
    type Config = Config;
}

impl<C: Collection> WithStartAndShutdown for OriginMuxer<C> {
    fn is_running(&self) -> bool {
        self.status.blocking_lock().as_ref().unwrap().is_running()
    }

    async fn start(&self) {
        let mut guard = self.status.lock().await;
        let status = guard.take().unwrap();
        let next_status = if let Status::NotRunning { muxer } = status {
            let (handle, shutdown) = muxer.spawn();
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
            let muxer = match handle.await {
                Ok(muxer) => muxer,
                Err(e) => {
                    std::panic::resume_unwind(e.into_panic());
                },
            };
            Status::NotRunning { muxer }
        } else {
            status
        };
        *guard = Some(next_status);
    }
}

impl<C: Collection> OriginProviderInterface<C> for OriginMuxer<C> {
    fn init(config: Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        let (socket, rx) = Socket::raw_bounded(2048);

        let mut muxer = Muxer::new(rx);

        muxer.http_origin(HttpOriginFetcher::<C>::init(
            config.http,
            blockstore.clone(),
        )?);

        muxer.ipfs_origin(IPFSOrigin::<C>::init(config.ipfs, blockstore.clone())?);

        Ok(Self {
            status: Mutex::new(Some(Status::NotRunning { muxer })),
            socket,
        })
    }

    fn get_socket(&self) -> OriginProviderSocket {
        self.socket.clone()
    }
}
