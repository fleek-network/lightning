use anyhow::{anyhow, bail};
use aya::maps::{MapData, RingBuf};
use lightning_ebpf_common::{EventMessage, EVENT_MESSAGE_LEN};
use log::{debug, error, info};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::io::unix::AsyncFd;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::config::ConfigSource;
use crate::connection::Connection;
use crate::event::write_events;
use crate::frame::{PACKET_FILTER_SERVICE, SUB_SECURITY_TOPIC, SUB_STATS_TOPIC};
use crate::map::SharedMap;
use crate::utils;

pub struct Server {
    listener: UnixListener,
    shared_state: SharedMap,
    config_src: ConfigSource,
    events: AsyncFd<RingBuf<MapData>>,
}

impl Server {
    pub fn new(
        listener: UnixListener,
        shared_state: SharedMap,
        config_src: ConfigSource,
        events: RingBuf<MapData>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            listener,
            shared_state,
            config_src,
            events: AsyncFd::new(events)?,
        })
    }

    async fn handle_watcher_event(&mut self, event: Event) -> anyhow::Result<()> {
        info!("received event: {event:?}");
        if event.kind.is_modify() {
            if event
                .paths
                .iter()
                .map(|path| path.as_path())
                .any(|p| p == self.config_src.packet_filers_path())
            {
                self.shared_state.update_packet_filters().await?;
            } else if event
                .paths
                .iter()
                .map(|path| path.as_path())
                .any(|p| p.parent() == Some(self.config_src.profiles_path()))
            {
                for path in event.paths {
                    self.shared_state.update_file_rules(path).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_new_stream(&mut self, stream: UnixStream) -> anyhow::Result<()> {
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
                let _ = watch_event_tx.blocking_send(res);
            },
            Config::default(),
        )?;

        watcher.watch(
            self.config_src.packet_filers_path().parent().unwrap(),
            RecursiveMode::NonRecursive,
        )?;
        watcher.watch(self.config_src.profiles_path(), RecursiveMode::NonRecursive)?;

        self.shared_state.update_packet_filters().await?;
        self.shared_state.update_all_file_rules().await?;

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
                            if let Err(err) = self.handle_watcher_event(event).await {
                                error!("error while handling watcher event: {err:?}")
                            }
                        }
                        Err(err) => {
                            error!("watcher returned an error: {err:?}")
                        }
                    }
                }
                // Todo: Verify that this is cancel safe.
                next = self.events.readable_mut() => {
                    let mut guard = next?;
                    let queue = guard.get_inner_mut();
                    let mut events: Vec<EventMessage> = Vec::with_capacity(EVENT_MESSAGE_LEN);
                    while let Some(bytes) = queue.next() {
                        match (*bytes).try_into() {
                            Ok(event) => {
                                debug!("received event {:?}", bytes);
                                events.push(event);
                            }
                            Err(e) => {
                                error!("failed to parse bytes from event queue: {e:?}");
                            }
                        }
                    }

                    if let Err(e) = write_events(events).await {
                        error!("failed to write event records: {e:?}");
                    }

                    guard.clear_ready();
                }
            }
        }
    }
}
