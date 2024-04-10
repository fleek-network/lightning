use log::{error, info};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::net::UnixListener;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::connection::Connection;
use crate::map::storage::Storage;
use crate::map::SharedStateMap;

pub struct Server {
    listener: UnixListener,
    shared_state: SharedStateMap,
    storage: Storage,
}

impl Server {
    pub fn new(listener: UnixListener, shared_state: SharedStateMap) -> Self {
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
                            let shared_state = self.shared_state.clone();
                            tokio::spawn(async move {
                                if let Err(e) =
                                    Connection::new(stream, shared_state).handle().await {
                                    info!("connection handler failed: {e:?}");
                                }
                            });
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
