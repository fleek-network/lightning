mod lookup;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task::JoinSet,
};

use crate::{
    query::{NodeInfo, Response},
    table::{TableKey, TableRequest},
    task::lookup::LookupTask,
};

pub async fn start_worker(
    mut rx: Receiver<Task>,
    mut network_event: Receiver<ResponseEvent>,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
) {
    let mut task_set = TaskManager {
        inflight: HashMap::new(),
        set: JoinSet::new(),
        local_key: NodeNetworkingPublicKey(rand::random()),
        table_tx,
        socket,
    };
    loop {
        select! {
            task = rx.recv() => {
                let task = task.expect("all channels to not drop");
                task_set.execute(task);
            }
            event = network_event.recv() => {
                let event = event.expect("all channels to not drop");
                task_set.handle_response(event);
            }
            response = task_set.set.join_next() => {
                todo!()
            }
        }
    }
}

pub enum Task {
    Bootstrap,
    Lookup {
        target: TableKey,
        tx: oneshot::Sender<TaskResponse>,
    },
    Ping {
        target: TableKey,
        rx: oneshot::Sender<()>,
    },
}

#[derive(Default)]
pub struct TaskResponse {
    pub nodes: Vec<NodeInfo>,
    pub value: Option<Vec<u8>>,
    pub rtt: Option<usize>,
    pub source: Option<NodeNetworkingPublicKey>,
}

struct TaskManager {
    inflight: HashMap<u64, OngoingTask>,
    set: JoinSet<Result<TaskResponse>>,
    local_key: NodeNetworkingPublicKey,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
}

impl TaskManager {
    fn handle_response(&self, event: ResponseEvent) {
        match self.inflight.get(&event.id) {
            Some(ongoing) => {
                if ongoing.tx.is_closed() {
                    // The task is done so this request is not expected.
                    tracing::warn!("received unexpected response");
                    return;
                }
                let task_tx = ongoing.tx.clone();
                tokio::spawn(async move {
                    if task_tx.send(event).await.is_err() {
                        tracing::error!("tasked dropped ")
                    }
                });
            },
            None => {
                tracing::warn!("received unexpected response");
            },
        }
    }

    fn execute(&mut self, task: Task) {
        let id: u64 = rand::random();
        match task {
            Task::Lookup { target, tx } => {
                let (task_tx, task_rx) = mpsc::channel(20);
                self.inflight.insert(
                    id,
                    OngoingTask {
                        response_tx: tx,
                        tx: task_tx,
                    },
                );
                let lookup = LookupTask::new(
                    id,
                    false,
                    self.local_key,
                    target,
                    self.table_tx.clone(),
                    task_rx,
                    self.socket.clone(),
                );
                self.set.spawn(async move { lookup::lookup(lookup).await });
            },
            Task::Bootstrap => todo!(),
            Task::Ping { .. } => todo!(),
        }
    }
}

struct OngoingTask {
    /// Send network event to task.
    tx: Sender<ResponseEvent>,
    /// Send back response to client.
    response_tx: oneshot::Sender<TaskResponse>,
}

#[derive(Debug)]
pub struct ResponseEvent {
    pub id: u64,
    pub sender_key: NodeNetworkingPublicKey,
    pub response: Response,
}
