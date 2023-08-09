use std::collections::HashMap;

use tokio::sync::{mpsc::Receiver, oneshot};

use crate::table::TableKey;

pub enum StoreRequest {
    Get {
        key: TableKey,
        tx: oneshot::Sender<Option<Vec<u8>>>,
    },
    Put {
        key: TableKey,
        value: Vec<u8>,
    },
}

pub async fn start_worker(mut rx: Receiver<StoreRequest>) {
    let mut storage = HashMap::new();
    while let Some(request) = rx.recv().await {
        match request {
            StoreRequest::Get { key, tx } => {
                let value = storage.get(&key).cloned();
                if tx.send(value).is_err() {
                    tracing::warn!("[Store]: client dropped channel")
                }
            },
            StoreRequest::Put { key, value } => {
                tracing::trace!("storing {key:?}:{value:?}");
                storage.insert(key, value);
            },
        }
    }
}
