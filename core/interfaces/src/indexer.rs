use infusion::c;
use lightning_types::Blake3Hash;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    KeystoreInterface,
    SignerInterface,
};

#[infusion::service]
pub trait IndexerInterface<C: Collection>: ConfigConsumer + Clone + Send + Sync + Sized {
    fn _init(
        config: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        keystore: ::KeystoreInterface,
        signer: ::SignerInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(),
            keystore.clone(),
            signer,
        )
    }

    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        keystore: C::KeystoreInterface,
        signer: &c!(C::SignerInterface),
    ) -> anyhow::Result<Self>;

    async fn register(&self, cid: Blake3Hash);

    async fn unregister(&self, cid: Blake3Hash);
}
