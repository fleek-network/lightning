use log::{error, info};
use notify::{Config, RecommendedWatcher, Watcher};
use tokio::net::UnixListener;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::connection::Connection;
use crate::state::SharedStateMap;

pub struct Server {
    listener: UnixListener,
    shared_state: SharedStateMap,
}

impl Server {
    pub fn new(listener: UnixListener, shared_state: SharedStateMap) -> Self {
        Self {
            listener,
            shared_state,
        }
    }

    pub async fn start(self) -> anyhow::Result<()> {
        let (watch_event_tx, mut watch_event_rx) = mpsc::channel(96);
        let watcher = RecommendedWatcher::new(
            move |res| {
                tokio::task::block_in_place(|| {
                    Handle::current().block_on(async {
                        watch_event_tx.send(res).await.unwrap();
                    });
                })
            },
            Config::default(),
        )?;

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
                        Ok(event) => {}
                        Err(err) => {
                            error!("watcher returned an error: {err:?}")
                        }
                    }
                }
            }
        }
    }
}
