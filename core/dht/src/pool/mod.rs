mod client;
mod lookup;
mod worker;

use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
pub use client::Client;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::oneshot;
pub use worker::WorkerPool;

use crate::table::server::TableKey;

pub type ValueRespond = oneshot::Sender<anyhow::Result<Option<Bytes>>>;
pub type ContactRespond = oneshot::Sender<anyhow::Result<Vec<NodeIndex>>>;

pub trait Pool {
    fn lookup_value(&self, key: TableKey, respond: ValueRespond) -> Result<()>;

    fn lookup_contact(&self, key: TableKey, respond: ContactRespond) -> Result<()>;

    fn store(&self, key: TableKey, value: Bytes) -> Result<()>;

    fn ping(&self, dst: NodeIndex, timeout: Duration) -> Result<()>;
}
