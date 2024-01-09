use lightning_types::Blake3Hash;

use crate::infu_collection::Collection;
use crate::{ConfigConsumer, ConfigProviderInterface, SignerInterface, SubmitTxSocket};

#[infusion::service]
pub trait IndexerInterface<C: Collection>: ConfigConsumer + Clone + Send + Sync + Sized {
    fn _init(config: ::ConfigProviderInterface, signer: ::SignerInterface) {
        Self::init(config.get::<Self>(), signer.get_socket())
    }

    fn init(config: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self>;

    fn register(&self, cid: Blake3Hash);

    fn unregister(&self, cid: Blake3Hash);
}
