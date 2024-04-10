use std::marker::PhantomData;

use affair::{Executor, TokioSpawn};
use lightning_interfaces::prelude::*;

use crate::config::ForwarderConfig;
use crate::worker::Worker;

pub struct Forwarder<C> {
    socket: MempoolSocket,
    _p: PhantomData<C>,
}

impl<C: Collection> ForwarderInterface<C> for Forwarder<C> {
    fn mempool_socket(&self) -> MempoolSocket {
        self.socket.clone()
    }
}

impl<C> ConfigConsumer for Forwarder<C> {
    const KEY: &'static str = "forwarder";
    type Config = ForwarderConfig;
}

impl<C: Collection> BuildGraph for Forwarder<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            |keystore: &C::KeystoreInterface, app: &C::ApplicationInterface| {
                let consensus_key = keystore.get_bls_pk();
                let query_runner = app.sync_query();
                let socket = TokioSpawn::spawn_async(Worker::new(consensus_key, query_runner));

                Self {
                    socket,
                    _p: PhantomData,
                }
            },
        )
    }
}
