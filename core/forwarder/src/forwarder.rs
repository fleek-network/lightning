use std::marker::PhantomData;

use affair::{Executor, TokioSpawn};
use anyhow::Result;
use fleek_crypto::ConsensusPublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    ForwarderInterface,
    MempoolSocket,
};

use crate::config::ForwarderConfig;
use crate::worker::Worker;

pub struct Forwarder<C> {
    socket: MempoolSocket,
    _p: PhantomData<C>,
}

impl<C: Collection> ForwarderInterface<C> for Forwarder<C> {
    fn init(
        _config: Self::Config,
        consensus_key: ConsensusPublicKey,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Result<Self> {
        let socket = TokioSpawn::spawn_async(Worker::new(consensus_key, query_runner));

        Ok(Self {
            socket,
            _p: PhantomData,
        })
    }

    fn mempool_socket(&self) -> MempoolSocket {
        self.socket.clone()
    }
}

impl<C> ConfigConsumer for Forwarder<C> {
    const KEY: &'static str = "forwarder";
    type Config = ForwarderConfig;
}
