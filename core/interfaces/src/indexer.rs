use infusion::c;
use lightning_types::Blake3Hash;

use crate::infu_collection::Collection;
use crate::{ApplicationInterface, ConfigConsumer, ConfigProviderInterface, SignerInterface};

#[infusion::service]
pub trait IndexerInterface<C: Collection>: ConfigConsumer + Clone + Send + Sync + Sized {
    fn _init(
        config: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        signer: ::SignerInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(file!(), line!()),
            signer,
        )
    }

    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        signer: &c!(C::SignerInterface),
    ) -> anyhow::Result<Self>;

    fn register(&self, cid: Blake3Hash);

    fn unregister(&self, cid: Blake3Hash);
}
