use async_trait::async_trait;
use infusion::c;
use lightning_types::{KeyPrefix, TableEntry};

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    SignerInterface,
    WithStartAndShutdown,
};

#[async_trait]
#[infusion::service]
pub trait DhtInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Clone + Sized + Send + Sync
{
    fn _init(
        app: ::ApplicationInterface,
        signer: ::SignerInterface,
        config: ::ConfigProviderInterface,
    ) {
        Self::init(signer, app.sync_query(), config.get::<Self>())
    }

    fn init(
        signer: &c!(C::SignerInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        config: Self::Config,
    ) -> anyhow::Result<Self>;

    /// Put a key-value pair into the distributed hash table.
    fn put(&self, prefix: KeyPrefix, key: &[u8], value: &[u8]);

    /// Return one value associated with the given key.
    async fn get(&self, prefix: KeyPrefix, key: &[u8]) -> Option<TableEntry>;
}
