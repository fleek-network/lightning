use std::fs::read_to_string;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use fleek_crypto::{
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
    TransactionSender,
    TransactionSignature,
};
use lightning_application::genesis::Genesis;
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::ToDigest;
use lightning_node::config::TomlConfigProvider;
use lightning_rpc::{utils, Rpc};
use lightning_signer::Signer;
use lightning_types::{
    NodeInfo,
    Participation,
    TransactionRequest,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use reqwest::Client;
use resolved_pathbuf::ResolvedPathBuf;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::args::OptSubCmd;

pub async fn exec<C: Collection<RpcInterface = Rpc<C>, SignerInterface = Signer<C>>>(
    cmd: OptSubCmd,
    config_path: ResolvedPathBuf,
) -> Result<()> {
    match cmd {
        OptSubCmd::In => opt_in::<C>(config_path).await,
        OptSubCmd::Out => opt_out::<C>(config_path).await,
        OptSubCmd::Status => status::<C>(config_path).await,
    }
}

async fn opt_in<C: Collection<RpcInterface = Rpc<C>, SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();

    let genesis_committee = get_genesis_committee().context("Failed to load genesis committee.")?;

    let node_info =
        genesis_committee_rpc::<Option<NodeInfo>>(&genesis_committee, get_node_info(public_key))
            .await
            .context("Failed to get node info from genesis committee")?;

    let node_info = node_info.context("Node not found on state.")?;
    if node_info.participation == Participation::True {
        println!("Your node is already participating in the network.");
        return Ok(());
    }
    let tx = create_update_request(UpdateMethod::OptIn {}, secret_key, node_info.nonce + 1);

    genesis_committee_rpc::<()>(&genesis_committee, send_transaction(tx))
        .await
        .context("Failed to send transaction to genesis committee")?;

    wait_for_participation_status(&genesis_committee, public_key, Participation::OptedIn, 10)
        .await
        .context("Timed out while trying to confirm opt in transaction.")?;
    println!(
        "Opt in transaction was successful. Your node will be participating once the next epoch starts."
    );
    Ok(())
}

async fn opt_out<C: Collection<RpcInterface = Rpc<C>, SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();

    let genesis_committee = get_genesis_committee().context("Failed to load genesis committee.")?;

    let node_info =
        genesis_committee_rpc::<Option<NodeInfo>>(&genesis_committee, get_node_info(public_key))
            .await
            .context("Failed to get node info from genesis committee")?;
    let node_info = node_info.context("Node not found on state.")?;
    if node_info.participation == Participation::False {
        println!(
            "Your node is not participating in the network. Opting out of the network is not necessary."
        );
        return Ok(());
    }
    let tx = create_update_request(UpdateMethod::OptOut {}, secret_key, node_info.nonce + 1);

    genesis_committee_rpc::<()>(&genesis_committee, send_transaction(tx))
        .await
        .context("Failed to send transaction to genesis committee")?;

    wait_for_participation_status(&genesis_committee, public_key, Participation::OptedOut, 10)
        .await
        .context("Timed out while trying to confirm opt out transaction.")?;
    println!(
        "Opt out transaction was successful. Your can shutdown your node after the current epoch ends."
    );
    Ok(())
}

async fn status<C: Collection<RpcInterface = Rpc<C>, SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();

    let genesis_committee = get_genesis_committee()
        .context("Failed to get genesis committee via rpc. Is your node running?")?;

    let node_info =
        genesis_committee_rpc::<Option<NodeInfo>>(&genesis_committee, get_node_info(public_key))
            .await
            .context("Failed to get node info from genesis committee")?;
    let node_info = node_info.context("Node not found on state.")?;

    match node_info.participation {
        Participation::True => println!("Your node is participating in the network."),
        Participation::False => println!("Your node is not participating in the network."),
        Participation::OptedIn => {
            println!("Your node is opted in. Your node will be participating next epoch.")
        },
        Participation::OptedOut => {
            println!("Your node is opted out. You can shutdown your node next epoch.")
        },
    }

    Ok(())
}

fn load_secret_key<C: Collection<SignerInterface = Signer<C>>>(
    config: Arc<TomlConfigProvider<C>>,
) -> Result<NodeSecretKey> {
    let signer_config = config.get::<C::SignerInterface>();
    if signer_config.node_key_path.exists() {
        let node_secret_key =
            read_to_string(&signer_config.node_key_path).context("Failed to read node pem file")?;
        let node_secret_key = NodeSecretKey::decode_pem(&node_secret_key)
            .context("Failed to decode node pem file")?;
        Ok(node_secret_key)
    } else {
        Err(anyhow::anyhow!("Node public key does not exist"))
    }
}

fn create_update_request(
    method: UpdateMethod,
    secret_key: NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let payload = UpdatePayload {
        sender: TransactionSender::NodeMain(secret_key.to_pk()),
        nonce,
        method,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        payload,
        signature: TransactionSignature::NodeMain(signature),
    }
}

fn send_transaction(update_request: UpdateRequest) -> String {
    json!({
        "jsonrpc": "2.0",
        "method":"flk_send_txn",
        "params": {"tx": TransactionRequest::UpdateRequest(update_request)},
        "id":1,
    })
    .to_string()
}

fn get_node_info(public_key: NodePublicKey) -> String {
    json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_info",
        "params":{
            "public_key": public_key,
            "version": 3
        },
        "id":1,
    })
    .to_string()
}

fn get_genesis_committee() -> Result<Vec<NodeInfo>> {
    let genesis = Genesis::load()?;
    Ok(genesis
        .node_info
        .iter()
        .filter(|node| node.genesis_committee)
        .map(NodeInfo::from)
        .collect())
}

async fn genesis_committee_rpc<T: DeserializeOwned>(
    genesis_committee: &Vec<NodeInfo>,
    request: String,
) -> Result<T> {
    let client = Client::new();
    for node in genesis_committee {
        if let Ok(res) = utils::rpc_request::<T>(
            &client,
            format!("http://{}:{}/rpc/v0", node.domain, node.ports.rpc),
            request.clone(),
        )
        .await
        {
            return Ok(res.result);
        }
    }
    Err(anyhow::anyhow!(
        "Unable to get a response from genesis committee"
    ))
}

async fn wait_for_participation_status(
    genesis_committee: &Vec<NodeInfo>,
    public_key: NodePublicKey,
    target_status: Participation,
    max_tries: u32,
) -> Result<()> {
    for _ in 0..max_tries {
        let node_info =
            genesis_committee_rpc::<Option<NodeInfo>>(genesis_committee, get_node_info(public_key))
                .await?;
        let node_info = node_info.context("Node not found on state.")?;
        if node_info.participation == target_status {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    Err(anyhow::anyhow!(
        "Failed to get status after {max_tries} tries."
    ))
}
