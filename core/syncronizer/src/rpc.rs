use std::future::Future;

use anyhow::{anyhow, Result};
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesOrdered;
use futures::StreamExt;
use lightning_interfaces::types::{Epoch, EpochInfo, NodeIndex, NodeInfo};
use lightning_rpc::interface::Fleek;
use lightning_rpc::RpcClient;
use tokio::runtime::Handle;

/// Runs the given future to completion on the current tokio runtime.
/// This call is intentionally blocking.
pub fn sync_call<F>(fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send,
{
    let handle = Handle::current();
    std::thread::spawn(move || handle.block_on(fut))
        .join()
        .unwrap()
}

/// Returns the epoch info from the epoch the bootstrap nodes are on
pub async fn get_epoch_info(nodes: Vec<(NodeIndex, NodeInfo)>) -> Result<EpochInfo> {
    let mut epochs = ask_epoch_info(nodes).await;

    if epochs.is_empty() {
        return Err(anyhow!("Failed to get epoch info from bootstrap nodes"));
    }
    epochs.sort_by(|a, b| a.epoch.partial_cmp(&b.epoch).unwrap());
    Ok(epochs.pop().unwrap())
}

/// A list of the nodes reported epoch info
///
/// ### Empty if all the requests fail
pub async fn ask_epoch_info(nodes: Vec<(NodeIndex, NodeInfo)>) -> Vec<EpochInfo> {
    nodes
        .into_iter()
        .map(|(_, node)| async move {
            let client =
                RpcClient::new_no_auth(&format!("http://{}:{}", node.domain, node.ports.rpc))
                    .ok()?;

            client.get_epoch_info().await.ok()
        })
        .collect::<FuturesOrdered<_>>()
        .filter_map(std::future::ready)
        .collect::<Vec<_>>()
        .await
}

/// Returns the node info for our node, if it's already on the state.
pub async fn get_node_info(
    node_public_key: NodePublicKey,
    nodes: Vec<(NodeIndex, NodeInfo)>,
) -> Result<Option<NodeInfo>> {
    let mut node_info = ask_node_info(nodes, node_public_key).await;

    if node_info.is_empty() {
        return Err(anyhow!("Failed to get node info from bootstrap nodes"));
    }

    node_info.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(node_info.pop().unwrap().0)
}

/// A list of the nodes reported node info
///
/// ### Empty if all the requests fail
pub async fn ask_node_info(
    nodes: Vec<(NodeIndex, NodeInfo)>,
    pk: NodePublicKey,
) -> Vec<(Option<NodeInfo>, Epoch)> {
    nodes
        .into_iter()
        .map(|(_, node)| async move {
            let client =
                RpcClient::new_no_auth(&format!("http://{}:{}", node.domain, node.ports.rpc))
                    .ok()?;

            client.get_node_info_epoch(pk).await.ok()
        })
        .collect::<FuturesOrdered<_>>()
        .filter_map(std::future::ready)
        .collect::<Vec<_>>()
        .await
}

/// Returns the node info for our node, if it's already on the state.
pub async fn check_if_node_has_sufficient_stake(
    node_public_key: NodePublicKey,
    nodes: Vec<(NodeIndex, NodeInfo)>,
) -> Result<bool> {
    let mut is_valid = ask_if_node_has_sufficient_stake(nodes, node_public_key).await;

    if is_valid.is_empty() {
        return Err(anyhow!("Failed to get node validity from bootstrap nodes"));
    }
    is_valid.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(is_valid.pop().unwrap().0)
}

/// Ask the nodes if the given node is valid
///
/// ### Empty if all the requests fail
pub async fn ask_if_node_has_sufficient_stake(
    nodes: Vec<(NodeIndex, NodeInfo)>,
    pk: NodePublicKey,
) -> Vec<(bool, Epoch)> {
    nodes
        .into_iter()
        .map(|(_, node)| async move {
            let client =
                RpcClient::new_no_auth(&format!("http://{}:{}", node.domain, node.ports.rpc))
                    .ok()?;

            let epoch = client.get_epoch().await.ok()?;
            let has_sufficient_stake = client.node_has_sufficient_stake(pk).await.ok()?;
            Some((has_sufficient_stake, epoch))
        })
        .collect::<FuturesOrdered<_>>()
        .filter_map(std::future::ready)
        .collect::<Vec<_>>()
        .await
}

/// Returns the hash of the last epoch ckpt, and the current epoch.
pub async fn last_epoch_hash(nodes: &[(NodeIndex, NodeInfo)]) -> Result<[u8; 32]> {
    let mut hash = ask_last_epoch_hash(nodes.to_vec()).await;

    if hash.is_empty() {
        return Err(anyhow!(
            "Failed to get last epoch hash from bootstrap nodes"
        ));
    }
    hash.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(hash.pop().unwrap().0)
}

/// A list of the nodes reported last epoch hash
///
/// ### Empty if all the requests fail
pub async fn ask_last_epoch_hash(nodes: Vec<(NodeIndex, NodeInfo)>) -> Vec<([u8; 32], Epoch)> {
    nodes
        .into_iter()
        .map(|(_, node)| async move {
            let client =
                RpcClient::new_no_auth(&format!("http://{}:{}", node.domain, node.ports.rpc))
                    .ok()?;

            client.get_last_epoch_hash().await.ok()
        })
        .collect::<FuturesOrdered<_>>()
        .filter_map(std::future::ready)
        .collect::<Vec<_>>()
        .await
}

/// Returns the epoch info from the epoch the bootstrap nodes are on
///
/// ### Empty if all the requests fail
pub async fn get_epoch(nodes: Vec<(NodeIndex, NodeInfo)>) -> Result<Vec<Epoch>> {
    let results = nodes
        .into_iter()
        .map(|(_, node)| async move {
            let client =
                RpcClient::new_no_auth(&format!("http://{}:{}", node.domain, node.ports.rpc))
                    .ok()?;

            client.get_epoch().await.ok()
        })
        .collect::<FuturesOrdered<_>>()
        .filter_map(std::future::ready)
        .collect::<Vec<_>>()
        .await;

    if results.is_empty() {
        Err(anyhow!("Unable to get a response from nodes"))
    } else {
        Ok(results)
    }
}
