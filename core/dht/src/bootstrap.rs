use anyhow::{anyhow, Result};
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot,
};

use crate::{
    bucket::{Node, MAX_BUCKETS},
    handler::HandlerCommand,
    query::NodeInfo,
    table::{TableCommand, TableKey},
};

pub enum BootstrapCommand {
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
    mut server_rx: Receiver<BootstrapCommand>,
    table_tx: Sender<TableCommand>,
    handler_tx: Sender<HandlerCommand>,
    local_key: NodeNetworkingPublicKey,
    bootstrap_nodes: Vec<NodeInfo>,
) {
    let mut state = State::Idle;
    loop {
        while let Some(message) = server_rx.recv().await {
            match message {
                BootstrapCommand::Start => {
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
                BootstrapCommand::DoneBootstrapping { tx } => {
                    if tx.send(state == State::Bootstrapped).is_err() {
                        tracing::error!("bootstrap client dropped the channel");
                    }
                },
                BootstrapCommand::Shutdown => {
                    tracing::trace!("shutting down bootstrap worker");
                    return;
                },
            }
        }
    }
}

async fn bootstrap(
    handler_tx: Sender<HandlerCommand>,
    table_tx: Sender<TableCommand>,
    boostrap_nodes: &[NodeInfo],
    local_key: NodeNetworkingPublicKey,
) -> Result<()> {
    for node in boostrap_nodes.iter() {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableCommand::AddNode {
                node: Node { info: node.clone() },
                tx,
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
        .send(TableCommand::FirstNonEmptyBucket { tx })
        .await
        .expect("table worker not to drop channel");
    let mut index = rx
        .await
        .expect("table worker not to drop channel")
        .ok_or_else(|| anyhow!("failed to find next bucket"))?;

    let mut target;

    while index < MAX_BUCKETS {
        target = random_key_in_bucket(index);
        closest_nodes(target, handler_tx.clone(), table_tx.clone()).await?;
        index += 1
    }

    Ok(())
}

pub async fn closest_nodes(
    target: NodeNetworkingPublicKey,
    handler_tx: Sender<HandlerCommand>,
    table_tx: Sender<TableCommand>,
) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    handler_tx
        .send(HandlerCommand::FindNode { target, tx })
        .await
        .expect("dispatcher worker not to drop channel");
    let nodes = rx.await.expect("dispatcher worker not to drop channel")?;

    for node in nodes {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableCommand::AddNode {
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
