use std::{collections::HashMap, sync::Arc};

use tokio::sync::{mpsc::Receiver, oneshot, Notify};

use crate::table::TableKey;

pub enum StoreRequest {
    Get {
        key: TableKey,
        respond: oneshot::Sender<Option<Vec<u8>>>,
    },
    Put {
        key: TableKey,
        value: Vec<u8>,
    },
}

pub async fn start_worker(mut rx: Receiver<StoreRequest>, shutdown_notify: Arc<Notify>) {
    let mut storage = HashMap::new();
    loop {
        tokio::select! {
            request = rx.recv() => {
                if let Some(request) = request {
                    match request {
                        StoreRequest::Get { key, respond } => {
                            tracing::trace!("received GET {key:?}");
                            let value = storage.get(&key).cloned();
                            if respond.send(value).is_err() {
                                tracing::warn!("client dropped channel")
                            }
                        },
                        StoreRequest::Put { key, value } => {
                            tracing::trace!("storing {key:?}:{value:?}");
                            storage.insert(key, value);
                        },
                    }
                }
            }
            _ = shutdown_notify.notified() => {
                tracing::info!("shutting down handler");
                break;
            }
        }
    }
}
