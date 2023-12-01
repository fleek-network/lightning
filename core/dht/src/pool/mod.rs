mod lookup;
mod worker;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
pub use lookup::Looker;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, Notify};
pub use worker::PoolWorker;

use crate::network::UnreliableTransport;
use crate::pool::lookup::LookupInterface;
use crate::pool::worker::Task;
use crate::table::worker::TableKey;
use crate::table::{Event, Table};

pub type ValueRespond = oneshot::Sender<anyhow::Result<Option<Bytes>>>;
pub type ContactRespond = oneshot::Sender<anyhow::Result<Vec<NodeIndex>>>;

pub fn create_pool_and_worker<L, T, U>(
    us: NodeIndex,
    looker: L,
    table: T,
    socket: U,
    event_queue: Sender<Event>,
    shutdown: Arc<Notify>,
) -> (DhtPool, PoolWorker<L, T, U>)
where
    L: LookupInterface,
    T: Table,
    U: UnreliableTransport,
{
    let (task_queue_tx, task_queue_rx) = mpsc::channel(1024);
    let max_pool_size = 2048;
    let worker = PoolWorker::new(
        us,
        looker,
        socket,
        table,
        task_queue_rx,
        event_queue,
        max_pool_size,
        shutdown,
    );
    (
        DhtPool {
            task_queue: task_queue_tx,
        },
        worker,
    )
}

pub trait Pool {
    fn lookup_value(&self, key: TableKey, respond: ValueRespond) -> Result<()>;

    fn lookup_contact(&self, key: TableKey, respond: ContactRespond) -> Result<()>;

    fn store(&self, key: TableKey, value: Bytes) -> Result<()>;

    fn ping(&self, dst: NodeIndex, timeout: Duration) -> Result<()>;
}

pub struct DhtPool {
    task_queue: Sender<Task>,
}

impl DhtPool {
    pub fn new(task_queue: Sender<Task>) -> Self {
        Self { task_queue }
    }
}

impl Clone for DhtPool {
    fn clone(&self) -> Self {
        Self {
            task_queue: self.task_queue.clone(),
        }
    }
}

// Todo: we may want to consider adding backpressure to these methods
// which is why these methods return a result.
impl Pool for DhtPool {
    fn lookup_value(&self, key: TableKey, respond: ValueRespond) -> Result<()> {
        let queue = self.task_queue.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::LookUpValue { key, respond }).await;
        });
        Ok(())
    }

    fn lookup_contact(&self, key: TableKey, respond: ContactRespond) -> Result<()> {
        let queue = self.task_queue.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::LookUpNode { key, respond }).await;
        });
        Ok(())
    }

    fn store(&self, key: TableKey, value: Bytes) -> Result<()> {
        let queue = self.task_queue.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::Store { key, value }).await;
        });
        Ok(())
    }

    fn ping(&self, dst: NodeIndex, timeout: Duration) -> Result<()> {
        let queue = self.task_queue.clone();
        tokio::spawn(async move {
            let _ = queue.send(Task::Ping { dst, timeout }).await;
        });
        Ok(())
    }
}
