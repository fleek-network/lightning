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
use lightning_interfaces::config::ConfigProviderInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::ToDigest;
use lightning_node::config::TomlConfigProvider;
use lightning_rpc::server::Rpc;
use lightning_rpc::utils;
use lightning_signer::Signer;
use lightning_types::{NodeInfo, TransactionRequest, UpdateMethod, UpdatePayload, UpdateRequest};
use reqwest::Client;
use resolved_pathbuf::ResolvedPathBuf;
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
    let port = get_rpc_port(config);

    let nonce = get_node_nonce(public_key, port)
        .await
        .context("Failed to query nonce via rpc. Is your node running?")?;

    let tx = create_update_request(UpdateMethod::OptIn {}, secret_key, nonce + 1);
    send_transaction(tx, port)
        .await
        .context("Failed to send opt in transaction. Is your node running?")?;

    wait_for_participation_status(public_key, port, true, 10)
        .await
        .context("Timed out while trying to confirm opt in transaction.")?;
    println!("Opt in transaction was successful.");
    Ok(())
}

async fn opt_out<C: Collection<RpcInterface = Rpc<C>, SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();
    let port = get_rpc_port(config);

    let nonce = get_node_nonce(public_key, port)
        .await
        .context("Failed to query nonce via rpc. Is your node running?")?;

    let tx = create_update_request(UpdateMethod::OptOut {}, secret_key, nonce + 1);
    send_transaction(tx, port)
        .await
        .context("Failed to send opt out transaction. Is your node running?")?;

    wait_for_participation_status(public_key, port, true, 10)
        .await
        .context("Timed out while trying to confirm opt out transaction.")?;
    println!("Opt out transaction was successful. You can shutdown your node.");
    Ok(())
}

async fn status<C: Collection<RpcInterface = Rpc<C>, SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();
    let port = get_rpc_port(config);

    let status = get_participation_status(public_key, port)
        .await
        .context("Failed to query participation status. Is your node running?")?;

    if status {
        println!("Your node is participating in the network.");
    } else {
        println!("Your node is not participating in the network.");
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
        Err(anyhow::anyhow!("Node Public Key: does not exist"))
    }
}

fn get_rpc_port<C: Collection<RpcInterface = Rpc<C>>>(config: Arc<TomlConfigProvider<C>>) -> u16 {
    config.get::<C::RpcInterface>().port
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

async fn get_node_nonce(node_public_key: NodePublicKey, port: u16) -> Result<u64> {
    let node_info = get_node_info(node_public_key, port).await?;
    Ok(node_info.nonce)
}

async fn send_transaction(update_request: UpdateRequest, port: u16) -> Result<()> {
    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_send_txn",
        "params": TransactionRequest::UpdateRequest(update_request),
        "id":1,
    })
    .to_string();

    let client = Client::new();
    utils::rpc_request::<()>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        request.to_string(),
    )
    .await?;

    Ok(())
}

async fn get_participation_status(node_public_key: NodePublicKey, port: u16) -> Result<bool> {
    let node_info = get_node_info(node_public_key, port).await?;
    Ok(node_info.participating)
}

async fn get_node_info(node_public_key: NodePublicKey, port: u16) -> Result<NodeInfo> {
    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_info",
        "params":{
            "public_key": node_public_key,
            "version": 3
        },
        "id":1,
    })
    .to_string();

    let client = Client::new();
    let node_info = utils::rpc_request::<NodeInfo>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        request.to_string(),
    )
    .await?;
    Ok(node_info.result)
}

async fn wait_for_participation_status(
    node_public_key: NodePublicKey,
    port: u16,
    target_status: bool,
    max_tries: u32,
) -> Result<()> {
    for _ in 0..max_tries {
        let status = get_participation_status(node_public_key, port).await?;
        if status == target_status {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    Err(anyhow::anyhow!(
        "Failed to get status after {max_tries} tries."
    ))
}
