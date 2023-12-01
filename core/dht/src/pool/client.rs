use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::Sender;

use crate::pool::worker::Task;
use crate::pool::{ContactRespond, Pool, ValueRespond};
use crate::table::server::TableKey;

pub struct Client {
    inner: Sender<Task>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// Todo: we may want to consider adding backpressure to these methods
// which is why these methods return a result.
impl Pool for Client {
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
