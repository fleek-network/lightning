use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
use bytes::Bytes;
use deadqueue::limited::Queue;
use futures::stream::FuturesUnordered;
use tokio::net::UdpSocket;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::SyncQueryRunnerInterface;
use crate::v2::lookup::LookupInterface;

pub enum Requests {
    Get,
    Put,
    Bootstrap,
}

pub struct NetworkMessage {
    id: u64,
    payload: Bytes,
}

// 1. Manage tasks from user.
// 2. Read messages from incoming-queue which are messages received from the network.
//      - These include tasks like FIND_NODE or STORE_VALUE.
//      - Some include RPC responses that must be passed on to ongoing tasks.
// 3. (Needs thinking because this might be able to be done by Table itself) Schedule routine tasks related to pruning table.
pub struct TaskManager<C: Collection, L: LookupInterface<C>> {
    /// Look-up task.
    lookup_task: L,
    /// Messages received from the network.
    incoming_message: Queue<NetworkMessage>,
    /// Requests from user.
    requests: Queue<Requests>,
    /// Ongoing tasks.
    ongoing_tasks: FuturesUnordered<JoinHandle<u64>>,
    /// Ongoing lookup tasks.
    ongoing: HashMap<u64, ()>,
    /// Maximum pool size.
    max_pool_size: usize,
    /// Shutdown notify.
    shutdown: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection, L: LookupInterface<C>> TaskManager<C, L> {
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