use std::collections::HashSet;

use anyhow::{anyhow, Result};
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task::JoinSet,
};

use crate::{
    bucket::MAX_BUCKETS,
    handler::HandlerRequest,
    query::NodeInfo,
    table::{TableKey, TableRequest},
};

#[allow(unused)]
pub enum BootstrapRequest {
    Start,
    DoneBootstrapping { tx: oneshot::Sender<bool> },
    Shutdown,
}

#[derive(Clone, PartialEq)]
pub enum State {
    Idle,
    Bootstrapping,
    Bootstrapped,
}

pub async fn start_worker(
    mut server_rx: Receiver<BootstrapRequest>,
    table_tx: Sender<TableRequest>,
    handler_tx: Sender<HandlerRequest>,
    local_key: NodeNetworkingPublicKey,
    bootstrap_nodes: Vec<NodeInfo>,
) {
    let mut state = State::Idle;
    loop {
        while let Some(request) = server_rx.recv().await {
            match request {
                BootstrapRequest::Start => {
                    // If we have no bootstrap nodes, it means we are the bootstrap node.
                    // Thus, we mark our node as already bootstrapped.
                    if bootstrap_nodes.is_empty() {
                        state = State::Bootstrapped;
                        continue;
                    }

                    if state != State::Bootstrapping {
                        match bootstrap(
                            handler_tx.clone(),
                            table_tx.clone(),
                            &bootstrap_nodes,
                            local_key,
                        )
                        .await
                        {
                            Ok(()) => {
                                state = State::Bootstrapped;
                            },
                            Err(e) => {
                                state = State::Idle;
                                tracing::error!("failed to start bootstrapping: {e}")
                            },
                        }
                    }
                },
                BootstrapRequest::DoneBootstrapping { tx } => {
                    if tx.send(state == State::Bootstrapped).is_err() {
                        tracing::error!("client dropped the channel");
                    }
                },
                BootstrapRequest::Shutdown => {
                    tracing::trace!("shutting down bootstrap worker");
                    return;
                },
            }
        }
    }
}

async fn bootstrap(
    handler_tx: Sender<HandlerRequest>,
    table_tx: Sender<TableRequest>,
    boostrap_nodes: &[NodeInfo],
    local_key: NodeNetworkingPublicKey,
) -> Result<()> {
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

    closest_nodes(local_key, handler_tx.clone(), table_tx.clone()).await?;

    let (tx, rx) = oneshot::channel();
    table_tx
        .send(TableRequest::FirstNonEmptyBucket { tx })
        .await
        .expect("table worker not to drop channel");
    let index = rx
        .await
        .expect("table worker not to drop channel")
        .ok_or_else(|| anyhow!("failed to find next bucket"))?;

    let mut search_list = (index..MAX_BUCKETS)
        .map(random_key_in_bucket)
        .collect::<HashSet<_>>();

    let mut task_set = JoinSet::new();

    // We start our lookup tasks in parallel.
    for target in search_list.iter() {
        let handler_tx = handler_tx.clone();
        let table_tx = table_tx.clone();
        let target = *target;
        task_set.spawn(async move {
            if let Err(e) = closest_nodes(target, handler_tx, table_tx).await {
                tracing::error!("failed to find closest nodes: {e}");
            }
            target
        });
    }

    while let Some(target) = task_set.join_next().await {
        // The spawned task returns the target even if it failed so unwrap is Ok.
        search_list.remove(&target.unwrap());
    }

    Ok(())
}

pub async fn closest_nodes(
    target: NodeNetworkingPublicKey,
    handler_tx: Sender<HandlerRequest>,
    table_tx: Sender<TableRequest>,
) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    handler_tx
        .send(HandlerRequest::FindNode { target, tx })
        .await
        .expect("handler worker not to drop channel");
    let nodes = rx.await.expect("handler worker not to drop channel")?;

    for node in nodes {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableRequest::AddNode { node, tx: Some(tx) })
            .await
            .expect("table worker not to drop channel");
        if let Err(e) = rx.await.expect("table worker not to drop channel") {
            tracing::error!("unexpected error while querying table: {e:?}");
        }
    }

    Ok(())
}

pub fn random_key_in_bucket(mut index: usize) -> NodeNetworkingPublicKey {
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
    NodeNetworkingPublicKey(key)
}
