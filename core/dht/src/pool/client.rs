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
    pub fn lookup_value(&self, _key: TableKey, _respond: ValueRespond) -> Result<()> {
        Ok(())
    }

    pub fn lookup_contact(&self, _key: TableKey, _respond: ContactRespond) -> Result<()> {
        Ok(())
    }

    pub fn store(&self, _key: TableKey, _value: Bytes) -> Result<()> {
        Ok(())
    }

    pub fn ping(&self, _dst: NodeIndex) -> Result<()> {
        Ok(())
    }
}
