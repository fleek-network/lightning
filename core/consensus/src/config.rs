use multiaddr::Multiaddr;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// The address in which the primary will listen for incoming requests on. This MUST
    /// be a UDP address.
    pub address: Multiaddr,
    /// UDP address which the worker is using to connect with the other workers and the
    /// primary.
    pub worker_address: Multiaddr,
    /// TCP address which the worker is listening on to receive transactions from user space.
    pub mempool_address: Multiaddr,
    /// Path to the database used by the narwhal implementation.
    pub store_path: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        // TODO(qti3e) We should decide on the default ports. I used the following format:
        // reserve 8xxx for consensus layer in the entire ursa project.
        // 8000 for primary
        // 8x01 for worker `x` address
        // 8x02 for worker `x` transaction address
        // 8003 for ABCI rpc server

        Self {
            address: "/ip4/0.0.0.0/udp/8000".parse().unwrap(),
            worker_address: "/ip4/0.0.0.0/udp/8001".parse().unwrap(),
            mempool_address: "/ip4/0.0.0.0/udp/8002".parse().unwrap(),
            store_path: "~/.fleek/data/narwhal_store"
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}
