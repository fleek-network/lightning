#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "service")]
mod connection;
#[cfg(any(feature = "client", feature = "service"))]
mod schema;
#[cfg(feature = "service")]
pub mod server;
#[cfg(feature = "service")]
mod state;
