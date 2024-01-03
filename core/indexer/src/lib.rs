mod config;

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, ContentUpdate, UpdateMethod};
use lightning_interfaces::{
    ConfigConsumer,
    IndexerInterface,
    SubmitTxSocket,
    WithStartAndShutdown,
};

use crate::config::Config;

pub struct Indexer {
    submit_tx: SubmitTxSocket,
    is_running: AtomicBool,
}

impl ConfigConsumer for Indexer {
    const KEY: &'static str = "indexer";
    type Config = Config;
}

#[async_trait]
impl WithStartAndShutdown for Indexer {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);
    }

    async fn shutdown(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

impl<C: Collection> IndexerInterface<C> for Indexer {
    fn init(_: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        Ok(Self {
            submit_tx,
            is_running: AtomicBool::new(false),
        })
    }

    fn register(&self, cids: Vec<Blake3Hash>) {
        let updates = cids
            .into_iter()
            .map(|cid| ContentUpdate { cid, remove: false })
            .collect();
        let submit_tx = self.submit_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = submit_tx
                .run(UpdateMethod::UpdateContentRegistry { updates })
                .await
            {
                tracing::error!("Submitting content registry update failed: {e:?}");
            }
        });
    }

    fn unregister(&self, cids: Vec<Blake3Hash>) {
        let updates = cids
            .into_iter()
            .map(|cid| ContentUpdate { cid, remove: true })
            .collect();
        let submit_tx = self.submit_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = submit_tx
                .run(UpdateMethod::UpdateContentRegistry { updates })
                .await
            {
                tracing::error!("Submitting content registry removal update failed: {e:?}");
            }
        });
    }
}
