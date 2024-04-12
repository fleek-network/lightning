use anyhow::{anyhow, bail};
use log::{error, info};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::net::{UnixListener, UnixStream};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::config::ConfigSource;
use crate::connection::Connection;
use crate::frame::{PACKET_FILTER_SERVICE, SUB_SECURITY_TOPIC, SUB_STATS_TOPIC};
use crate::map::SharedMap;
use crate::utils;

pub struct Server {
    listener: UnixListener,
    shared_state: SharedMap,
    storage: ConfigSource,
}

impl Server {
    pub fn new(listener: UnixListener, shared_state: SharedMap) -> Self {
        Self {
            listener,
            shared_state,
            storage: Default::default(),
        }
    }

    pub fn handle_watcher_event(&mut self, event: Event) -> anyhow::Result<()> {
        info!("received event: {event:?}");
        if event.kind.is_modify() {
            if event
                .paths
                .iter()
                .map(|path| path.as_path())
                .any(|p| p == self.storage.packet_filers_path())
            {
            } else if event
                .paths
                .iter()
                .map(|path| path.as_path())
                .any(|p| p == self.storage.profiles_path())
            {
            }
        }

        Ok(())
    }

    pub async fn handle_new_stream(&mut self, stream: UnixStream) -> anyhow::Result<()> {
        let shared_state = self.shared_state.clone();
        let service = utils::read_service_header(&stream)
            .await?
            .ok_or(anyhow!("socket was closed unexpectedly"))?;
        match service {
            PACKET_FILTER_SERVICE => {
                tokio::spawn(async move {
                    if let Err(e) = Connection::new(stream, shared_state).handle().await {
                        info!("connection handler failed: {e:?}");
                    }
                });
            },
            SUB_SECURITY_TOPIC | SUB_STATS_TOPIC => {
                // Todo: add new subscriber.
            },
            _ => bail!("invalid service header"),
        }

        Ok(())
    }

    pub async fn start(mut self) -> anyhow::Result<()> {
        let (watch_event_tx, mut watch_event_rx) = mpsc::channel(96);
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                tokio::task::block_in_place(|| {
                    Handle::current().block_on(async {
                        let _ = watch_event_tx.send(res).await;
                    });
                })
            },
            Config::default(),
        )?;

        watcher.watch(
            self.storage.packet_filers_path(),
            RecursiveMode::NonRecursive,
        )?;
        watcher.watch(self.storage.profiles_path(), RecursiveMode::NonRecursive)?;

        loop {
            tokio::select! {
                next = self.listener.accept() => {
                    match next {
                        Ok((stream, _addr)) => {
                            if let Err(e) = self.handle_new_stream(stream).await {
                                error!("failed to handle the new stream: {e:?}");
                            }
                        },
                        Err(e) => {
                            error!("accept failed: {e:?}")
                        },
                    }
                }
                next = watch_event_rx.recv() => {
                    match next.expect("Watcher not to drop") {
                        Ok(event) => {
                            if let Err(err) = self.handle_watcher_event(event) {
                                error!("error while handling watcher event: {err:?}")
                            }
                        }
                        Err(err) => {
                            error!("watcher returned an error: {err:?}")
                        }
                    }
                }
            }
        }
    }
}
