use async_trait::async_trait;
use lightning_types::ImmutablePointer;

use crate::infu_collection::Collection;
use crate::{
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    OriginProviderInterface,
    ResolverInterface,
};

#[async_trait]
#[infusion::service]
pub trait FetcherInterface<C: Collection>: ConfigConsumer + Clone + Sized + Send + Sync {
    fn _init(
        config: ::ConfigProviderInterface,
        blockstore: ::BlockStoreInterface,
        resolver: ::ResolverInterface,
        origin: ::OriginProviderInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            blockstore.clone(),
            resolver.clone(),
            origin,
        )
    }

    /// Initialize the fetcher.
    fn init(
        config: Self::Config,
        blockstore: C::BlockStoreInterface,
        resolver: C::ResolverInterface,
        origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self>;

    /// Fetches the data from the corresponding origin, puts it in the blockstore, and stores the
    /// mapping using the resolver. If the mapping already exists, the data will not be fetched
    /// from origin again.
    async fn put(&self, pointer: ImmutablePointer) -> Blake3Hash;

    /// Fetches the data from the blockstore. If the data does not exist in the blockstore, it will
    /// be fetched from the origin.
    async fn fetch(&self, hash: Blake3Hash) -> anyhow::Result<()>;
}
