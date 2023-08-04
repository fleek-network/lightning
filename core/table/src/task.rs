use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{
    bucket::Node,
    handler::Command,
    table::{TableKey, TableQuery},
};

pub async fn closest_nodes(
    target: NodeNetworkingPublicKey,
    command_tx: Sender<Command>,
    table_tx: Sender<TableQuery>,
) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    command_tx.send(Command::FindNode { target, tx }).await?;
    let nodes = rx.await?.unwrap();

    for node in nodes {
        let (tx, rx) = oneshot::channel();
        table_tx
            .send(TableQuery::AddNode {
                node: Node { info: node },
                tx,
            })
            .await
            .unwrap();
        if let Ok(Err(e)) = rx.await {
            tracing::error!("unexpected error while querying table: {e:?}");
        }
    }

    Ok(())
}

pub fn random_key_in_bucket(mut index: usize) -> NodeNetworkingPublicKey {
    let mut key: TableKey = rand::random();
    for mut byte in key.iter_mut() {
        if index > 7 {
            byte = &mut 0;
        } else {
            byte = &mut ((*byte | 128u8) >> index as u8);
            break;
        }
    }
    NodeNetworkingPublicKey(key)
}
