use anyhow::{anyhow, Result};
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot,
};

use crate::{
    bucket::{Node, MAX_BUCKETS},
    handler::Command,
    query::NodeInfo,
    table::{TableKey, TableQuery},
};

pub enum BootstrapRequest {
    Start { tx: oneshot::Sender<bool> },
    DoneBootstrapping { tx: oneshot::Sender<bool> },
    Shutdown,
}

#[derive(Clone, PartialEq)]
pub enum State {
    Initial,
    Bootstrapping,
    Bootstrapped,
}

pub async fn start_worker(
    mut server_rx: Receiver<BootstrapRequest>,
    table_tx: Sender<TableQuery>,
    command_tx: Sender<Command>,
    local_key: NodeNetworkingPublicKey,
    bootstrap_nodes: Vec<NodeInfo>,
) {
    let mut state = State::Initial;
    loop {
        while let Some(message) = server_rx.recv().await {
            match message {
                BootstrapRequest::Start { tx } => {
                    if state == State::Bootstrapping {
                        if tx.send(false).is_err() {
                            tracing::error!("bootstrap client dropped the channel");
                        }
                    } else {
                        match bootstrap(
                            command_tx.clone(),
                            table_tx.clone(),
                            &bootstrap_nodes,
                            local_key,
                        )
                        .await
                        {
                            Ok(()) => {
                                state = State::Bootstrapped;
                            },
                            Err(e) => tracing::error!("failed to start bootstrapping: {e}"),
                        }
                    }
                },
                BootstrapRequest::DoneBootstrapping { tx } => {
                    if tx.send(state == State::Bootstrapped).is_err() {
                        tracing::error!("bootstrap client dropped channel");
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
    command_tx: Sender<Command>,
    table_tx: Sender<TableQuery>,
    boostrap_nodes: &[NodeInfo],
    local_key: NodeNetworkingPublicKey,
) -> Result<()> {
    for node in boostrap_nodes.iter() {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableQuery::AddNode {
                node: Node { info: node.clone() },
                tx,
            })
            .await
            .expect("table worker not to drop channel");
        if let Err(e) = rx.await.expect("table worker not to drop channel") {
            tracing::error!("unexpected error while querying table: {e:?}");
        }
    }

    let mut target = local_key;
    closest_nodes(target, command_tx.clone(), table_tx.clone()).await?;

    let (tx, rx) = oneshot::channel();
    table_tx
        .send(TableQuery::FirstNonEmptyBucket { tx })
        .await
        .expect("table worker not to drop channel");
    let mut index = rx
        .await
        .expect("table worker not to drop channel")
        .ok_or_else(|| anyhow!("failed to find next bucket"))?;

    while index < MAX_BUCKETS {
        target = random_key_in_bucket(index);
        closest_nodes(target, command_tx.clone(), table_tx.clone()).await?;
        index += 1
    }

    Ok(())
}

pub async fn closest_nodes(
    target: NodeNetworkingPublicKey,
    command_tx: Sender<Command>,
    table_tx: Sender<TableQuery>,
) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    command_tx
        .send(Command::FindNode { target, tx })
        .await
        .expect("dispatcher worker not to drop channel");
    let nodes = rx
        .await
        .expect("dispatcher worker not to drop channel")
        .map_err(|_| anyhow!("failed to get closest nodes to {target:?}"))?;

    for node in nodes {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableQuery::AddNode {
                node: Node { info: node },
                tx,
            })
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
