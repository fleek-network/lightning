mod config;

#[cfg(test)]
mod tests;

use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, ContentUpdate, NodeIndex, UpdateMethod};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    IndexerInterface,
    KeystoreInterface,
    SignerInterface,
    SubmitTxSocket,
    SyncQueryRunnerInterface,
};

use crate::config::Config;

pub struct Indexer<C: Collection> {
    pk: NodePublicKey,
    local_index: Arc<OnceLock<NodeIndex>>,
    submit_tx: SubmitTxSocket,
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    _marker: PhantomData<C>,
}

impl<C: Collection> Clone for Indexer<C> {
    fn clone(&self) -> Self {
        Self {
            pk: self.pk,
            local_index: self.local_index.clone(),
            submit_tx: self.submit_tx.clone(),
            query_runner: self.query_runner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<C: Collection> ConfigConsumer for Indexer<C> {
    const KEY: &'static str = "indexer";
    type Config = Config;
}

impl<C: Collection> Indexer<C> {
    fn get_index(&self) -> Option<NodeIndex> {
        match self.local_index.get() {
            None => {
                if let Some(index) = self.query_runner.pubkey_to_index(&self.pk) {
                    let _ = self.local_index.set(index);
                    Some(index)
                } else {
                    None
                }
            },
            Some(index) => Some(*index),
        }
    }
}

impl<C: Collection> IndexerInterface<C> for Indexer<C> {
    fn init(
        _: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        keystore: C::KeystoreInterface,
        signer: &C::SignerInterface,
    ) -> anyhow::Result<Self> {
        let submit_tx = signer.get_socket();

        let pk = keystore.get_ed25519_pk();
        let local_index = OnceLock::new();
        if let Some(index) = query_runner.pubkey_to_index(&pk) {
            local_index.set(index).expect("Cell to be empty");
        }

        Ok(Self {
            pk,
            local_index: Arc::new(local_index),
            submit_tx,
            query_runner,
            _marker: PhantomData,
        })
    }

    async fn register(&self, cid: Blake3Hash) {
        if let Some(index) = self.get_index() {
            if self
                .query_runner
                .get_content_registry(&index)
                .map(|registry| !registry.contains(&cid))
                .unwrap_or(true)
            {
                let updates = vec![ContentUpdate { cid, remove: false }];
                if let Err(e) = self
                    .submit_tx
                    .enqueue(UpdateMethod::UpdateContentRegistry { updates })
                    .await
                {
                    tracing::error!("Submitting content registry update failed: {e:?}");
                }
            }
        }
    }

    async fn unregister(&self, cid: Blake3Hash) {
        if let Some(index) = self.get_index() {
            if self
                .query_runner
                .get_content_registry(&index)
                .map(|registry| registry.contains(&cid))
                .unwrap_or(false)
            {
                let updates = vec![ContentUpdate { cid, remove: true }];

                if let Err(e) = self
                    .submit_tx
                    .enqueue(UpdateMethod::UpdateContentRegistry { updates })
                    .await
                {
                    tracing::error!("Submitting content registry update failed: {e:?}");
                }
            }
        }
    }
}
