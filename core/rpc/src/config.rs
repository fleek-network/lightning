use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use lightning_types::FirewallConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub addr: SocketAddr,
    pub rpc_selection: RPCSelection,
    pub disallowed_methods: Option<Arc<Vec<String>>>,
    pub firewall: lightning_types::FirewallConfig,
    pub hmac_secret: Option<PathBuf>,
}

impl Into<FirewallConfig> for Config {
    fn into(self) -> FirewallConfig {
        self.firewall
    }
}

impl Config {
    pub fn new(
        addr: SocketAddr,
        rpc_selection: RPCSelection,
        disallowed_methods: Option<Vec<String>>,
        firewall: lightning_types::FirewallConfig,
        hmac_secret: Option<PathBuf>,
    ) -> Self {
        Self {
            hmac_secret,
            firewall,
            addr,
            rpc_selection,
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
            hmac_secret: None,
            addr: "0.0.0.0:4230".parse().expect("RPC Socket Addr to parse"),
            rpc_selection: Default::default(),
            disallowed_methods: None,
            firewall: Default::default(),
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
