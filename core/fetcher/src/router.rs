use lightning_interfaces::types::{Blake3Hash, ImmutablePointer, OriginProvider};
use lightning_interfaces::Collection;
use lightning_origin_b3fs::B3FSOrigin;
use lightning_origin_http::HttpOrigin;
use lightning_origin_ipfs::IPFSOrigin;

use crate::config::Config;

#[derive(Clone)]
pub(crate) struct Router<C: NodeComponents> {
    http: HttpOrigin<C>,
    ipfs: IPFSOrigin<C>,
    b3fs: B3FSOrigin<C>,
}

impl<C: NodeComponents> Router<C> {
    pub fn new(config: Config, blockstore: C::BlockstoreInterface) -> anyhow::Result<Self> {
        Ok(Self {
            http: HttpOrigin::<C>::new(config.http, blockstore.clone())?,
            ipfs: IPFSOrigin::<C>::new(config.ipfs, blockstore)?,
            b3fs: B3FSOrigin::<C>::new(config.b3fs)?,
        })
    }

    pub async fn route(&self, req: &ImmutablePointer) -> anyhow::Result<Blake3Hash> {
        match &req.origin {
            OriginProvider::HTTP => self.http.fetch(&req.uri).await,
            OriginProvider::IPFS => self.ipfs.fetch(&req.uri).await,
            OriginProvider::B3FS => self.b3fs.fetch(&req.uri).await,
            _ => Err(anyhow::anyhow!("unknown origin type")),
        }
    }
}
