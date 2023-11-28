use anyhow::Result;
use bytes::Bytes;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::Sender;

use crate::pool::{ContactRespond, Task, ValueRespond};
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

impl Client {
    pub fn lookup_value(&self, key: TableKey, respond: ValueRespond) -> Result<()> {
        Ok(())
    }

    pub fn lookup_contact(&self, key: TableKey, respond: ContactRespond) -> Result<()> {
        Ok(())
    }

    pub fn store(&self, key: TableKey, value: Bytes) -> Result<()> {
        Ok(())
    }

    pub fn ping(&self, hash: u32) -> Result<()> {
        Ok(())
    }
}
