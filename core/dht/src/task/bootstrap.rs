use std::collections::HashSet;

use anyhow::{anyhow, Result};
use tokio::{
    sync::{mpsc::Sender, oneshot},
    task::{JoinHandle, JoinSet},
};

use crate::{
    bucket::MAX_BUCKETS,
    query::NodeInfo,
    table::{TableKey, TableRequest},
    task::{Task, TaskFailed, TaskResult},
};

pub const BOOTSTRAP_TASK_ID: u64 = 0;

async fn bootstrap(
    task_tx: Sender<Task>,
    table_tx: Sender<TableRequest>,
    local_key: TableKey,
    nodes: Vec<NodeInfo>,
) -> Result<()> {
    if nodes.is_empty() {
        tracing::trace!("bootstrap complete");
        return Ok(());
    }

    let mut state = State::Idle;
    loop {
        match &mut state {
            State::Idle => {
                let task_tx = task_tx.clone();
                let table_tx = table_tx.clone();
                let nodes = nodes.clone();
                let local_key = local_key;
                let task = tokio::spawn(async move {
                    self_lookup(task_tx.clone(), table_tx.clone(), &nodes, local_key).await
                });
                state = State::Initial(task);
            },
            State::Initial(handle) => {
                let indexes = handle.await??;
                let mut set = JoinSet::new();
                for target in indexes {
                    let task_tx = task_tx.clone();
                    set.spawn(async move {
                        let (tx, rx) = oneshot::channel();
                        task_tx
                            .send(Task::Lookup {
                                target,
                                tx: Some(tx),
                                refresh_bucket: true,
                            })
                            .await
                            .expect("handler worker not to drop channel");
                        let _ = rx.await.expect("handler worker not to drop channel");
                        Ok(())
                    });
                }

                state = State::Lookups(set);
            },
            State::Lookups(set) => match set.join_next().await {
                Some(result) => {
                    if let Err(e) = result? {
                        tracing::error!("error occurred while bootstrapping: {e:?}");
                    }
                },
                None => {
                    state = State::Complete;
                },
            },
            State::Complete => break,
        }
    }

    Ok(())
}

pub async fn self_lookup(
    handler_tx: Sender<Task>,
    table_tx: Sender<TableRequest>,
    boostrap_nodes: &[NodeInfo],
    local_key: TableKey,
) -> Result<HashSet<TableKey>> {
    for node in boostrap_nodes.iter() {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableRequest::AddNode {
                node: node.clone(),
                tx: Some(tx),
            })
            .await
            .expect("table worker not to drop channel");
        if let Err(e) = rx.await.expect("table worker not to drop channel") {
            tracing::error!("unexpected error while querying table: {e:?}");
        }
    }

    let (tx, rx) = oneshot::channel();
    handler_tx
        .send(Task::Lookup {
            target: local_key,
            tx: Some(tx),
            refresh_bucket: true,
        })
        .await
        .expect("handler worker not to drop channel");
    let _ = rx.await.expect("handler worker not to drop channel");

    let (tx, rx) = oneshot::channel();
    table_tx
        .send(TableRequest::FirstNonEmptyBucket { tx })
        .await
        .expect("table worker not to drop channel");
    let index = rx
        .await
        .expect("table worker not to drop channel")
        .ok_or_else(|| anyhow!("failed to find next bucket"))?;

    let search_list = (index..MAX_BUCKETS)
        .map(random_key_in_bucket)
        .collect::<HashSet<_>>();

    Ok(search_list)
}

pub fn random_key_in_bucket(mut index: usize) -> TableKey {
    let mut key: TableKey = rand::random();
    for byte in key.iter_mut() {
        if index > 7 {
            *byte = 0;
        } else {
            *byte = (*byte | 128u8) >> index as u8;
            break;
        }
        index -= 8;
    }
    key
}

pub enum State {
    Idle,
    Initial(JoinHandle<Result<HashSet<TableKey>>>),
    Lookups(JoinSet<Result<()>>),
    Complete,
}

#[derive(Clone)]
pub struct Bootstrapper {
    task_tx: Sender<Task>,
    table_tx: Sender<TableRequest>,
    local_key: TableKey,
    nodes: Vec<NodeInfo>,
}

impl Bootstrapper {
    pub fn new(
        task_tx: Sender<Task>,
        table_tx: Sender<TableRequest>,
        local_key: TableKey,
        nodes: Vec<NodeInfo>,
    ) -> Self {
        Self {
            task_tx,
            table_tx,
            local_key,
            nodes,
        }
    }
    pub async fn start(self, response_tx: oneshot::Sender<Result<()>>) -> TaskResult {
        match bootstrap(self.task_tx, self.table_tx, self.local_key, self.nodes).await {
            Ok(_) => {
                if response_tx.send(Ok(())).is_err() {
                    tracing::error!("failed to send bootstrap result");
                }
                Ok(BOOTSTRAP_TASK_ID)
            },
            Err(error) => {
                if response_tx
                    .send(Err(anyhow::anyhow!("failed to bootstrap: {error:?}")))
                    .is_err()
                {
                    tracing::error!("failed to send bootstrap result");
                }
                Err(TaskFailed {
                    id: BOOTSTRAP_TASK_ID,
                    error,
                })
            },
        }
    }
}
