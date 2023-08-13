use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::{ConfigConsumer, SyncQueryRunnerInterface, TopologyInterface};

use super::config::Config;

pub struct Topology<Q: SyncQueryRunnerInterface> {
    query: PhantomData<Q>,
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> TopologyInterface for Topology<Q> {
    type SyncQuery = Q;

    fn init(
        _config: Self::Config,
        _our_public_key: NodePublicKey,
        _query_runner: Self::SyncQuery,
    ) -> anyhow::Result<Self> {
        Ok(Self { query: PhantomData })
    }

    fn suggest_connections(&self) -> Arc<Vec<Vec<NodePublicKey>>> {
        todo!()
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Topology<Q> {
    type Config = Config;

    const KEY: &'static str = "TOPOLOGY";
}
