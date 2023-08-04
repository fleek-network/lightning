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
    task,
};

pub enum Query {
    Start { tx: oneshot::Sender<bool> },
    DoneBootstrapping { tx: oneshot::Sender<bool> },
}

#[derive(Clone, PartialEq)]
pub enum State {
    Initial,
    Bootstrapping,
    Bootstrapped,
}

#[derive(Clone)]
struct Bootstrap {
    command_tx: Sender<Command>,
    // Send queries to table server.
    table_tx: Sender<TableQuery>,
    boostrap_nodes: Vec<NodeInfo>,
    local_key: NodeNetworkingPublicKey,
}

pub async fn start_server(
    mut server_rx: Receiver<Query>,
    table_tx: Sender<TableQuery>,
    command_tx: Sender<Command>,
    local_key: NodeNetworkingPublicKey,
    bootstrap_nodes: Vec<NodeInfo>,
) {
    let mut state = State::Initial;
    loop {
        while let Some(message) = server_rx.recv().await {
            match message {
                Query::Start { tx } => {
                    if state == State::Bootstrapping {
                        tx.send(false).unwrap();
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
                Query::DoneBootstrapping { tx } => {
                    if tx.send(state == State::Bootstrapped).is_err() {
                        tracing::error!("bootstrap client dropped channel");
                    }
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
            .unwrap();
        if let Ok(Err(e)) = rx.await {
            tracing::error!("unexpected error while querying table: {e:?}");
        }
    }

    let mut target = local_key;
    task::closest_nodes(target, command_tx.clone(), table_tx.clone()).await?;

    let (tx, rx) = oneshot::channel();
    table_tx
        .send(TableQuery::FirstNonEmptyBucket { tx })
        .await
        .unwrap();
    let mut index = rx
        .await?
        .ok_or_else(|| anyhow!("failed to find next bucket"))?;

    while index < MAX_BUCKETS {
        target = task::random_key_in_bucket(index);
        task::closest_nodes(target, command_tx.clone(), table_tx.clone()).await?;
        index += 1
    }

    Ok(())
}
