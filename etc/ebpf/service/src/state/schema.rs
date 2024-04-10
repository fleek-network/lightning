use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Filter {
    pub ip: Ipv4Addr,
    pub port: u16,
}
