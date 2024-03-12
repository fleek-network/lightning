use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    addr: SocketAddr,
    rpc_selection: RPCSelection,
}

impl Config {
    pub fn new(addr: SocketAddr, rpc_selection: RPCSelection) -> Self {
        Self {
            addr,
            rpc_selection,
        }
    }

    pub fn default_with_port_and_addr(addr: String, port: u16) -> Self {
        Self {
            addr: format!("{}:{}", addr, port)
                .parse()
                .expect("RPC Socket Addr to parse"),
            rpc_selection: Default::default(),
        }
    }

    pub fn default_with_port(port: u16) -> Self {
        Self {
            addr: format!("{}:{}", "0.0.0.0", port)
                .parse()
                .expect("RPC Socket Addr to parse"),
            rpc_selection: Default::default(),
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
            addr: "0.0.0.0:4240".parse().expect("RPC Socket Addr to parse"),
            rpc_selection: Default::default(),
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
