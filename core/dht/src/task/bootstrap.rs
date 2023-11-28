use std::collections::HashSet;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

use crate::node::NodeInfo;
use crate::table::bucket::{self, MAX_BUCKETS};
use crate::table::server::{TableKey, TableRequest};
use crate::task::{Task, TaskFailed, TaskResult};

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
                                respond: Some(tx),
                                refresh_bucket: true,
                                is_value: false,
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
                respond: Some(tx),
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
            respond: Some(tx),
            refresh_bucket: true,
            is_value: false,
        })
        .await
        .expect("handler worker not to drop channel");
    let _ = rx.await.expect("handler worker not to drop channel");

    let (tx, rx) = oneshot::channel();
    table_tx
        .send(TableRequest::NearestNeighborsBucket { respond: tx })
        .await
        .expect("table worker not to drop channel");
    let index = rx
        .await
        .expect("table worker not to drop channel")
        .ok_or_else(|| anyhow!("failed to find next bucket"))?;

    let search_list = (index..MAX_BUCKETS)
        .map(|index| bucket::random_key_in_bucket(index, &local_key))
        .collect::<HashSet<_>>();

    Ok(search_list)
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

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;
    use crate::distance;
    use crate::table::bucket::MAX_BUCKETS;

    #[test]
    fn test_random_key_in_bucket() {
        let local_key: TableKey = rand::random();
        let mut rng = rand::thread_rng();
        for _ in 0..10000 {
            let index = rng.gen_range(0..MAX_BUCKETS);
            let random_key = bucket::random_key_in_bucket(index, &local_key);
            let calc_index = distance::leading_zero_bits(&local_key, &random_key);
            assert_eq!(index, calc_index);
        }
    }
}
