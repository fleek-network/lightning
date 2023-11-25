use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use bytes::Bytes;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Notify;
use tokio::task::{JoinHandle, JoinSet};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use crate::v2::lookup::LookupInterface;

pub enum Task {
    LookUp {
        hash: u32,
        value: bool,
    },
    Ping {
        peer: NodeIndex,
        timeout: Duration,
    }
}

pub struct NetworkMessage {
    id: u64,
    payload: Bytes,
}

pub enum Event {
    Pong {
        peer: NodeIndex,
    },
    Timeout {
        peer: NodeIndex,
    }
}

// 1. Manage tasks from user.
// 2. Read messages from incoming-queue which are messages received from the network.
//      - These include tasks like FIND_NODE or STORE_VALUE.
//      - Some include RPC responses that must be passed on to ongoing tasks.
// 3. (Needs thinking because this might be able to be done by Table itself) Schedule routine tasks related to pruning table.
pub struct WorkerPool<C: Collection, L: LookupInterface<C>> {
    /// Look-up task.
    lookup_task: L,
    /// Requests from user.
    task_queue: Receiver<Task>,
    /// Queue to send ping events.
    ping_queue_tx: Sender<Event>,
    /// Ongoing tasks.
    ongoing_tasks: JoinSet<u64>,
    /// Ongoing lookup tasks.
    ongoing: HashMap<u64, ()>,
    /// Maximum pool size.
    max_pool_size: usize,
    /// Shutdown notify.
    shutdown: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection, L: LookupInterface<C>> WorkerPool<C, L> {
    pub async fn run(self) {
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.notified() => {}
                message = self.incoming_message.pop() => {}
                request = self.requests.pop() => {}
            }
        }
    }
}
