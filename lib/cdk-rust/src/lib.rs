//! # Lightning CDK for Rust.
//!
//! ## Primary Connection
//! ```ignore
//! use std::net::SocketAddr;
//!
//! use cdk_rust::transport::tcp::TcpTransport;
//! use cdk_rust::Builder;
//!
//! #[tokio::main]
//! async fn main() {
//!     let target: SocketAddr = "0.0.0.0:0".parse().unwrap();
//!     let transport = TcpTransport::new(target);
//!     let secret = [0u8; 32];
//!     let service_id = 1;
//!
//!     let connector = Builder::primary(secret, service_id)
//!         .transport(transport)
//!         .build()
//!         .unwrap();
//!
//!     let mut connection = connector.connect().await.unwrap();
//!
//!     let (ttl, token) = connection.request_access_token(1).await.unwrap();
//!     let (sender, receiver) = connection.split();
//! }
//! ```
//!
//! ## Secondary Connection
//!
//! ```ignore
//! use std::net::SocketAddr;
//!
//! use cdk_rust::transport::tcp::TcpTransport;
//! use cdk_rust::Builder;
//!
//! #[tokio::main]
//! async fn main() {
//!     let provider_pk = [0u8; 32];
//!     let target: SocketAddr = "0.0.0.0:0".parse().unwrap();
//!     let transport = TcpTransport::new(target);
//!     let token = [0u8; 48];
//!
//!     let connector = Builder::secondary(token, provider_pk)
//!         .transport(transport)
//!         .build()
//!         .unwrap();
//!
//!     let (sender, receiver) = connector.connect().await.unwrap().split();
//! }
//! ```

mod builder;
mod connection;
mod context;
mod frame;
mod mode;
#[cfg(not(feature = "cloudflare"))]
mod tls;

pub mod transport;

pub use builder::Builder;
pub use lightning_schema::handshake as schema;
