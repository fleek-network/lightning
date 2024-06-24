extern crate core;

#[cfg(feature = "server")]
mod connection;
#[cfg(feature = "client")]
pub mod filter;
#[cfg(feature = "server")]
pub mod server;

mod config;
#[cfg(feature = "server")]
mod event;
pub mod frame;
pub mod map;
mod utils;

pub use config::{ConfigSource, PathConfig};
