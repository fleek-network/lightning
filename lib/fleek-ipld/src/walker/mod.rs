//! This module contains the abstractions and implementations such that, given a `Cid`,
//! it can download and iterate over the subitems in that `Cid`.
//!
//! It contains differents modules to iterate, and walk through those elements in differents ways:
//!
//! - `stream.rs`: Contains the `IpldStream` struct that allows to iterate over the elements in a
//!   `Cid` in a stream fashion way, where
//! the user controls the continuation of the stream.
//! - `downloader.rs`: Contains the abstraction for downloading the data from IPFS, and also a
//!   default implementation using `reqwest` as the HTTP client.
//! - `data.rs`: Contains the abstractions to decode the data from IPFS into a specific format.
//! - `concurrent.rs`: Highly concurrent implementation, to iterate over the elements in a `Cid`
//!   that sends a signal to a callback function on each element explored.
pub mod concurrent;
pub mod data;
pub mod downloader;
pub mod stream;
