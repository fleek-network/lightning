mod config;

#[cfg(test)]
mod tests;

use std::marker::PhantomData;

use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, ContentUpdate, UpdateMethod};
use lightning_interfaces::{ConfigConsumer, IndexerInterface, SubmitTxSocket};

use crate::config::Config;

#[derive(Clone)]
pub struct Indexer<C> {
    submit_tx: SubmitTxSocket,
    _marker: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Indexer<C> {
    const KEY: &'static str = "indexer";
    type Config = Config;
}

impl<C: Collection> IndexerInterface<C> for Indexer<C> {
    fn init(_: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        Ok(Self {
            submit_tx,
            _marker: PhantomData,
        })
    }

    fn register(&self, cid: Blake3Hash) {
        let updates = vec![ContentUpdate { cid, remove: false }];
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

    fn unregister(&self, cid: Blake3Hash) {
        let updates = vec![ContentUpdate { cid, remove: true }];
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
