use std::marker::PhantomData;

use affair::AsyncWorker;
use lightning_interfaces::prelude::*;
use lightning_interfaces::{spawn_worker, ShutdownWaiter};

use crate::config::ForwarderConfig;
use crate::worker::Worker;

pub struct Forwarder<C> {
    socket: MempoolSocket,
    _p: PhantomData<C>,
}

impl<C: NodeComponents> ForwarderInterface<C> for Forwarder<C> {
    fn mempool_socket(&self) -> MempoolSocket {
        self.socket.clone()
    }
}

impl<C> ConfigConsumer for Forwarder<C> {
    const KEY: &'static str = "forwarder";
    type Config = ForwarderConfig;
}

impl<C: NodeComponents> BuildGraph for Forwarder<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            |keystore: &C::KeystoreInterface,
             app: &C::ApplicationInterface,
             fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>| {
                let consensus_key = keystore.get_bls_pk();
                let node_public_key = keystore.get_ed25519_pk();
                let query_runner = app.sync_query();
                let worker = Worker::new(consensus_key, node_public_key, query_runner);
                let socket = spawn_worker!(worker, "FORWARDER", waiter, crucial);

                Self {
                    socket,
                    _p: PhantomData,
                }
            },
        )
    }
}
