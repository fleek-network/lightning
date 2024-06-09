use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub addr: SocketAddr,
    pub rpc_selection: RPCSelection,
    pub max_connections: Option<usize>,
    pub blacklist: Option<Arc<Vec<IpAddr>>>,
    pub disallowed_methods: Option<Arc<Vec<String>>>,
}

impl Config {
    pub fn new(
        addr: SocketAddr,
        rpc_selection: RPCSelection,
        max_connections: Option<usize>,
        blacklist: Option<Vec<IpAddr>>,
        disallowed_methods: Option<Vec<String>>,
    ) -> Self {
        Self {
            addr,
            rpc_selection,
            max_connections,
            blacklist: blacklist.map(Arc::new),
            disallowed_methods: disallowed_methods.map(Arc::new),
        }
    }

    pub fn default_with_port_and_addr(addr: String, port: u16) -> Self {
        Self {
            addr: format!("{}:{}", addr, port)
                .parse()
                .expect("RPC Socket Addr to parse"),
            ..Default::default()
        }
    }

    pub fn default_with_port(port: u16) -> Self {
        Self {
            addr: format!("{}:{}", "0.0.0.0", port)
                .parse()
                .expect("RPC Socket Addr to parse"),
            ..Default::default()
        }
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:4230".parse().expect("RPC Socket Addr to parse"),
            rpc_selection: Default::default(),
            max_connections: None,
            blacklist: None,
            disallowed_methods: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RPCModules {
    Net,
    Eth,
    Flk,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RPCSelection {
    None,
    #[default]
    All,
    Some(Vec<RPCModules>),
}

impl RPCSelection {
    pub fn iter(self) -> impl Iterator<Item = RPCModules> {
        match self {
            RPCSelection::None => vec![].into_iter(),
            RPCSelection::All => vec![
                RPCModules::Net,
                RPCModules::Eth,
                RPCModules::Flk,
                RPCModules::Admin,
            ]
            .into_iter(),
            RPCSelection::Some(v) => v.into_iter(),
        }
    }
}

impl Config {
    pub fn rpc_selection(&self) -> impl Iterator<Item = RPCModules> {
        self.rpc_selection.clone().iter()
    }
}
