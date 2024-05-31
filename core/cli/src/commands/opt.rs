use std::io::stdin;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context, Result};
use fleek_crypto::{
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
    TransactionSender,
    TransactionSignature,
};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    ChainId,
    Epoch,
    EpochInfo,
    NodeInfo,
    Participation,
    TransactionRequest,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_utils::config::TomlConfigProvider;
use lightning_utils::rpc::rpc_request;
use resolved_pathbuf::ResolvedPathBuf;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::args::OptSubCmd;

pub async fn exec<C: Collection>(cmd: OptSubCmd, config_path: ResolvedPathBuf) -> Result<()> {
    match cmd {
        OptSubCmd::In => opt_in::<C>(config_path).await,
        OptSubCmd::Out => opt_out::<C>(config_path).await,
        OptSubCmd::Status => status::<C>(config_path).await,
    }
}

async fn opt_in<C: Collection>(config_path: ResolvedPathBuf) -> Result<()> {
    println!(
        "After sending the OptIn transaction, you are expected to start your node immediately. Is your node built and ready to start? (y/N)"
    );
    get_user_confirmation();

    let (public_key, secret_key) = load_secret_key::<C>(config_path).await?;

    let rpc_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let genesis_committee = C::ApplicationInterface::get_genesis_committee()
        .context("Failed to load genesis committee info.")?;

    let node_info = query_node_info(public_key, &genesis_committee, &rpc_client)
        .await
        .context("Failed to get node info from genesis committee")?;

    let node_info = node_info.context("Node not found on state.")?;
    if node_info.participation == Participation::True {
        println!("Your node is already participating in the network.");
        return Ok(());
    }

    let epoch_end_delta = get_epoch_end_delta(&genesis_committee, &rpc_client)
        .await
        .context("Failed to get epoch info from genesis committee")?;
    if epoch_end_delta < Duration::from_secs(300) {
        println!(
            "The current epoch will end in less than 5 minutes. Please wait until the epoch change
     to send the OptIn transaction."
        );
        return Ok(());
    }

    let chain_id =
        C::ApplicationInterface::get_chain_id().context("Failed to load Chain ID from genesis.")?;

    let tx = create_update_request(
        UpdateMethod::OptIn {},
        secret_key,
        node_info.nonce + 1,
        chain_id,
    );

    send_txn(tx, &genesis_committee, &rpc_client)
        .await
        .context("Failed to send transaction to genesis committee")?;

    println!("Confirming transaction...");
    wait_for_participation_status(
        &genesis_committee,
        public_key,
        Participation::OptedIn,
        10,
        &rpc_client,
    )
    .await
    .context("Timed out while trying to confirm opt in transaction.")?;
    println!(
        "Opt in transaction was successful. Your node will be participating once the next epoch starts."
    );
    Ok(())
}

async fn opt_out<C: Collection>(config_path: ResolvedPathBuf) -> Result<()> {
    println!(
        "Even after sending the OptOut transaction, your node is expected to stay online until the end of the current epoch. Do you want to continue? (y/N)"
    );
    get_user_confirmation();

    let (public_key, secret_key) = load_secret_key::<C>(config_path).await?;

    let rpc_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let genesis_committee = C::ApplicationInterface::get_genesis_committee()
        .context("Failed to load genesis committee info.")?;

    let node_info = query_node_info(public_key, &genesis_committee, &rpc_client)
        .await
        .context("Failed to get node info from genesis committee")?;
    let node_info = node_info.context("Node not found on state.")?;
    if node_info.participation == Participation::False {
        println!(
            "Your node is not participating in the network. Opting out of the network is not necessary."
        );
        return Ok(());
    }

    let chain_id =
        C::ApplicationInterface::get_chain_id().context("Failed to load Chain ID from genesis.")?;

    let tx = create_update_request(
        UpdateMethod::OptOut {},
        secret_key,
        node_info.nonce + 1,
        chain_id,
    );

    send_txn(tx, &genesis_committee, &rpc_client)
        .await
        .context("Failed to send transaction to genesis committee")?;

    println!("Confirming transaction...");
    wait_for_participation_status(
        &genesis_committee,
        public_key,
        Participation::OptedOut,
        10,
        &rpc_client,
    )
    .await
    .context("Timed out while trying to confirm opt out transaction.")?;
    println!(
        "Opt out transaction was successful. Your can shutdown your node after the current epoch ends."
    );
    Ok(())
}

async fn status<C: Collection>(config_path: ResolvedPathBuf) -> Result<()> {
    let (public_key, _secret_key) = load_secret_key::<C>(config_path).await?;

    let rpc_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let genesis_committee = C::ApplicationInterface::get_genesis_committee()
        .context("Failed to load genesis committee info.")?;

    let node_info = query_node_info(public_key, &genesis_committee, &rpc_client)
        .await
        .context("Failed to get node info from genesis committee")?;
    let node_info = node_info.context("Node not found on state.")?;

    match node_info.participation {
        Participation::True => {
            println!("Your node is participating in the network.");
            std::process::exit(0)
        },
        Participation::False => {
            println!("Your node is not participating in the network.");
            std::process::exit(210)
        },
        Participation::OptedIn => {
            let delta = get_epoch_end_delta(&genesis_committee, &rpc_client)
                .await
                .context("Failed to get epoch info from genesis committee")?;
            // Warning: The following text has a timestamp in the format HH:MM:SS that might be
            // parsed e.g. get.fleek.network. If you require to change, report it pls
            println!(
                "Your node is opted in. Your node will be participating once the next epoch starts in {}",
                get_timestamp(delta)
            );
            std::process::exit(211)
        },
        Participation::OptedOut => {
            let delta = get_epoch_end_delta(&genesis_committee, &rpc_client)
                .await
                .context("Failed to get epoch info from genesis committee")?;
            // Warning: The following text has a timestamp in the format HH:MM:SS that might be
            // parsed e.g. get.fleek.network. If you require to change, report it pls
            println!(
                "Your node is opted out. You can shutdown your node once the current epoch ends in {}",
                get_timestamp(delta)
            );
            std::process::exit(212)
        },
    }
}

async fn get_epoch_end_delta(
    genesis_committee: &[NodeInfo],
    rpc_client: &reqwest::Client,
) -> Result<Duration> {
    let epoch_info = query_epoch_info(genesis_committee, rpc_client)
        .await
        .context("Failed to get node info from genesis committee")?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let delta = (epoch_info.epoch_end).saturating_sub(now);
    Ok(Duration::from_millis(delta))
}

async fn load_secret_key<C: Collection>(
    config_path: ResolvedPathBuf,
) -> Result<(NodePublicKey, NodeSecretKey)> {
    let config_provider = TomlConfigProvider::<C>::load_or_write_config(config_path).await;
    let mut provider = fdi::Provider::default().with(config_provider);
    let mut g = C::build_graph();
    g.init_one::<C::KeystoreInterface>(&mut provider)?;
    let keystore = provider.get::<C::KeystoreInterface>();
    let sk = keystore.get_ed25519_sk();
    Ok((sk.to_pk(), sk))
}

fn create_update_request(
    method: UpdateMethod,
    secret_key: NodeSecretKey,
    nonce: u64,
    chain_id: ChainId,
) -> UpdateRequest {
    let payload = UpdatePayload {
        sender: TransactionSender::NodeMain(secret_key.to_pk()),
        nonce,
        method,
        chain_id,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        payload,
        signature: TransactionSignature::NodeMain(signature),
    }
}

pub async fn query_node_info(
    public_key: NodePublicKey,
    nodes: &[NodeInfo],
    rpc_client: &reqwest::Client,
) -> Result<Option<NodeInfo>> {
    let mut node_info: Vec<((Option<NodeInfo>, Epoch), String)> =
        query_genesis_committee(get_node_info(public_key), nodes, rpc_client).await?;

    if node_info.is_empty() {
        return Err(anyhow!("Failed to get node info from bootstrap nodes"));
    }
    node_info.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(node_info.pop().unwrap().0.0)
}

pub async fn query_epoch_info(
    nodes: &[NodeInfo],
    rpc_client: &reqwest::Client,
) -> Result<EpochInfo> {
    let mut epochs: Vec<(EpochInfo, String)> =
        query_genesis_committee(get_epoch_info(), nodes, rpc_client).await?;
    if epochs.is_empty() {
        return Err(anyhow!("Failed to get epoch info from bootstrap nodes"));
    }
    epochs.sort_by(|(a, _), (b, _)| a.epoch.partial_cmp(&b.epoch).unwrap());
    Ok(epochs.pop().unwrap().0)
}

pub async fn send_txn(
    update_request: UpdateRequest,
    nodes: &[NodeInfo],
    rpc_client: &reqwest::Client,
) -> Result<()> {
    let mut epochs: Vec<(EpochInfo, String)> =
        query_genesis_committee(get_epoch_info(), nodes, rpc_client).await?;
    if epochs.is_empty() {
        return Err(anyhow!("Failed to get epoch info from bootstrap nodes"));
    }

    // Pick a bootstrap node that is on the highest epoch
    epochs.sort_by(|(a, _), (b, _)| a.epoch.partial_cmp(&b.epoch).unwrap());
    let rpc_address = epochs.pop().unwrap().1;

    rpc_request::<()>(rpc_client, rpc_address, send_transaction(update_request))
        .await
        .map(|_| ())
}

pub async fn query_genesis_committee<T: DeserializeOwned>(
    req: String,
    nodes: &[NodeInfo],
    rpc_client: &reqwest::Client,
) -> Result<Vec<(T, String)>> {
    let mut futs = Vec::new();
    for node in nodes {
        let req_clone = req.clone();
        let fut = async move {
            let address = format!("http://{}:{}/rpc/v0", node.domain, node.ports.rpc);
            match rpc_request::<T>(rpc_client, address.clone(), req_clone).await {
                Ok(res) => Some((res, address)),
                Err(_) => None,
            }
        };
        futs.push(fut);
    }

    let results: Vec<(T, String)> = futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
        .map(|(x, address)| (x.result, address))
        .collect();

    if results.is_empty() {
        Err(anyhow!("Unable to get a response from nodes"))
    } else {
        Ok(results)
    }
}

async fn wait_for_participation_status(
    genesis_committee: &[NodeInfo],
    public_key: NodePublicKey,
    target_status: Participation,
    max_tries: u32,
    rpc_client: &reqwest::Client,
) -> Result<()> {
    for _ in 0..max_tries {
        let node_info = query_node_info(public_key, genesis_committee, rpc_client)
            .await
            .context("Failed to get node info from genesis committee")?;
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

fn get_user_confirmation() {
    loop {
        let mut input = String::new();
        match stdin().read_line(&mut input) {
            Ok(len) => match &input[0..len - 1] {
                "y" => break,
                "N" => {
                    println!("Not sending transaction.");
                    std::process::exit(1);
                },
                _ => {
                    println!(
                        "Invalid input, please type `y` for yes or `N` for no, and hit ENTER."
                    );
                },
            },
            Err(e) => {
                eprintln!("Failed to read from stdin: {e:?}");
                std::process::exit(1);
            },
        }
    }
}

fn get_timestamp(duration: Duration) -> String {
    let s = duration.as_secs() % 60;
    let m = (duration.as_secs() / 60) % 60;
    let h = (duration.as_secs() / 60) / 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
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
        "method":"flk_get_node_info_epoch",
        "params":{
            "public_key": public_key,
            "version": 3
        },
        "id":1,
    })
    .to_string()
}

fn get_epoch_info() -> String {
    json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch_info",
        "params":[],
        "id":1,
    })
    .to_string()
}
