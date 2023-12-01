mod lookup;
mod worker;

use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
pub use worker::PoolWorker;

use crate::pool::worker::Task;
use crate::table::worker::TableKey;

pub type ValueRespond = oneshot::Sender<anyhow::Result<Option<Bytes>>>;
pub type ContactRespond = oneshot::Sender<anyhow::Result<Vec<NodeIndex>>>;

pub trait Pool {
    fn lookup_value(&self, key: TableKey, respond: ValueRespond) -> Result<()>;

    fn lookup_contact(&self, key: TableKey, respond: ContactRespond) -> Result<()>;

    fn store(&self, key: TableKey, value: Bytes) -> Result<()>;

    fn ping(&self, dst: NodeIndex, timeout: Duration) -> Result<()>;
}

pub struct DhtPool {
    inner: Sender<Task>,
}

impl Clone for DhtPool {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// Todo: we may want to consider adding backpressure to these methods
// which is why these methods return a result.
impl Pool for DhtPool {
    fn lookup_value(&self, key: TableKey, respond: ValueRespond) -> Result<()> {
        let queue = self.inner.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::LookUpValue { key, respond }).await;
        });
        Ok(())
    }

    fn lookup_contact(&self, key: TableKey, respond: ContactRespond) -> Result<()> {
        let queue = self.inner.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::LookUpNode { key, respond }).await;
        });
        Ok(())
    }

    fn store(&self, key: TableKey, value: Bytes) -> Result<()> {
        let queue = self.inner.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::Store { key, value }).await;
        });
        Ok(())
    }

    fn ping(&self, dst: NodeIndex, timeout: Duration) -> Result<()> {
        let queue = self.inner.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::Ping { dst, timeout }).await;
        });
        Ok(())
    }
}
