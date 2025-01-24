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
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_rpc::api::RpcClient;
use lightning_rpc::interface::Fleek;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::OptSubCmd;

pub async fn exec<C: NodeComponents>(cmd: OptSubCmd, config_path: ResolvedPathBuf) -> Result<()> {
    match cmd {
        OptSubCmd::In => opt_in::<C>(config_path).await,
        OptSubCmd::Out => opt_out::<C>(config_path).await,
        OptSubCmd::Status => status::<C>(config_path).await,
    }
}

async fn opt_in<C: NodeComponents>(config_path: ResolvedPathBuf) -> Result<()> {
    println!(
        "After sending the OptIn transaction, you are expected to start your node immediately. Is your node built and ready to start? (y/N)"
    );
    get_user_confirmation();

    let config = TomlConfigProvider::<C>::load(config_path)?;
    let app_config = config.get::<<C as NodeComponents>::ApplicationInterface>();

    let (public_key, secret_key) = load_secret_key::<C>(config).await?;

    let genesis_committee = C::ApplicationInterface::get_genesis_committee(&app_config)
        .context("Failed to load genesis committee info.")?;

    let node_info = query_node_info(public_key, &genesis_committee)
        .await
        .context("Failed to get node info from genesis committee")?;

    let node_info = node_info.context("Node not found on state.")?;
    if node_info.participation == Participation::True {
        println!("Your node is already participating in the network.");
        return Ok(());
    }

    let epoch_end_delta = get_epoch_end_delta(&genesis_committee)
        .await
        .context("Failed to get epoch info from genesis committee")?;
    if epoch_end_delta < Duration::from_secs(300) {
        println!(
            "The current epoch will end in less than 5 minutes. Please wait until the epoch change
     to send the OptIn transaction."
        );
        return Ok(());
    }

    let chain_id = C::ApplicationInterface::get_chain_id(&app_config)
        .context("Failed to load Chain ID from genesis.")?;

    let tx = create_update_request(
        UpdateMethod::OptIn {},
        secret_key,
        node_info.nonce + 1,
        chain_id,
    );

    send_txn(tx, &genesis_committee)
        .await
        .context("Failed to send transaction to genesis committee")?;

    println!("Confirming transaction...");
    wait_for_participation_status(&genesis_committee, public_key, Participation::OptedIn, 10)
        .await
        .context("Timed out while trying to confirm opt in transaction.")?;
    println!(
        "Opt in transaction was successful. Your node will be participating once the next epoch starts."
    );
    Ok(())
}

async fn opt_out<C: NodeComponents>(config_path: ResolvedPathBuf) -> Result<()> {
    println!(
        "Even after sending the OptOut transaction, your node is expected to stay online until the end of the current epoch. Do you want to continue? (y/N)"
    );
    get_user_confirmation();

    let config = TomlConfigProvider::<C>::load(config_path)?;
    let app_config = config.get::<<C as NodeComponents>::ApplicationInterface>();

    let (public_key, secret_key) = load_secret_key::<C>(config).await?;

    let genesis_committee = C::ApplicationInterface::get_genesis_committee(&app_config)
        .context("Failed to load genesis committee info.")?;

    let node_info = query_node_info(public_key, &genesis_committee)
        .await
        .context("Failed to get node info from genesis committee")?;
    let node_info = node_info.context("Node not found on state.")?;
    if node_info.participation == Participation::False {
        println!(
            "Your node is not participating in the network. Opting out of the network is not necessary."
        );
        return Ok(());
    }

    let chain_id = C::ApplicationInterface::get_chain_id(&app_config)
        .context("Failed to load Chain ID from genesis.")?;

    let tx = create_update_request(
        UpdateMethod::OptOut {},
        secret_key,
        node_info.nonce + 1,
        chain_id,
    );

    send_txn(tx, &genesis_committee)
        .await
        .context("Failed to send transaction to genesis committee")?;

    println!("Confirming transaction...");
    wait_for_participation_status(&genesis_committee, public_key, Participation::OptedOut, 10)
        .await
        .context("Timed out while trying to confirm opt out transaction.")?;
    println!(
        "Opt out transaction was successful. Your can shutdown your node after the current epoch ends."
    );
    Ok(())
}

async fn status<C: NodeComponents>(config_path: ResolvedPathBuf) -> Result<()> {
    let config = TomlConfigProvider::<C>::load(config_path)?;
    let app_config = config.get::<<C as NodeComponents>::ApplicationInterface>();

    let (public_key, _secret_key) = load_secret_key::<C>(config).await?;

    let genesis_committee = C::ApplicationInterface::get_genesis_committee(&app_config)
        .context("Failed to load genesis committee info.")?;

    let node_info = query_node_info(public_key, &genesis_committee)
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
            let delta = get_epoch_end_delta(&genesis_committee)
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
            let delta = get_epoch_end_delta(&genesis_committee)
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

async fn get_epoch_end_delta(genesis_committee: &[NodeInfo]) -> Result<Duration> {
    let epoch_info = query_epoch_info(genesis_committee)
        .await
        .context("Failed to get node info from genesis committee")?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let delta = (epoch_info.epoch_end).saturating_sub(now);
    Ok(Duration::from_millis(delta))
}

async fn load_secret_key<C: NodeComponents>(
    config: TomlConfigProvider<C>,
) -> Result<(NodePublicKey, NodeSecretKey)> {
    let mut provider = fdi::Provider::default().with(config);
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
) -> Result<Option<NodeInfo>> {
    let mut node_info: Vec<((Option<NodeInfo>, Epoch), String)> =
        get_node_info_from_genesis_commitee(nodes, public_key).await?;

    if node_info.is_empty() {
        return Err(anyhow!("Failed to get node info from bootstrap nodes"));
    }
    node_info.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(node_info.pop().unwrap().0 .0)
}

pub async fn query_epoch_info(nodes: &[NodeInfo]) -> Result<EpochInfo> {
    let mut epochs: Vec<(EpochInfo, String)> = get_epoch_info_from_genesis_commitee(nodes).await?;

    if epochs.is_empty() {
        return Err(anyhow!("Failed to get epoch info from bootstrap nodes"));
    }
    epochs.sort_by(|(a, _), (b, _)| a.epoch.partial_cmp(&b.epoch).unwrap());
    Ok(epochs.pop().unwrap().0)
}

pub async fn send_txn(update_request: UpdateRequest, nodes: &[NodeInfo]) -> Result<()> {
    let mut epochs: Vec<(EpochInfo, String)> = get_epoch_info_from_genesis_commitee(nodes).await?;

    if epochs.is_empty() {
        return Err(anyhow!("Failed to get epoch info from bootstrap nodes"));
    }

    // Pick a bootstrap node that is on the highest epoch
    epochs.sort_by(|(a, _), (b, _)| a.epoch.partial_cmp(&b.epoch).unwrap());
    let rpc_address = epochs.pop().unwrap().1;

    let client = RpcClient::new_no_auth(&rpc_address)?;

    Ok(Fleek::send_txn(&client, update_request.into())
        .await
        .map(|_| ())?)
}

pub async fn get_node_info_from_genesis_commitee(
    nodes: &[NodeInfo],
    public_key: NodePublicKey,
) -> Result<Vec<((Option<NodeInfo>, Epoch), String)>> {
    let mut futs = Vec::new();
    for node in nodes {
        let fut = async move {
            let address = format!("http://{}:{}/rpc/v0", node.domain, node.ports.rpc);
            let client = RpcClient::new_no_auth(&address)
                .inspect_err(|e| {
                    tracing::error!("failed to create client for {} \n {:?}", address.clone(), e)
                })
                .ok()?;

            Some((
                Fleek::get_node_info_epoch(&client, public_key).await.ok()?,
                address,
            ))
        };
        futs.push(fut);
    }

    let results: Vec<((Option<NodeInfo>, Epoch), String)> = futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
        .collect();

    if results.is_empty() {
        Err(anyhow!("Unable to get a response from nodes"))
    } else {
        Ok(results)
    }
}

pub async fn get_epoch_info_from_genesis_commitee(
    nodes: &[NodeInfo],
) -> Result<Vec<(EpochInfo, String)>> {
    let mut futs = Vec::new();
    for node in nodes {
        let fut = async move {
            let address = format!("http://{}:{}/rpc/v0", node.domain, node.ports.rpc);
            let client = RpcClient::new_no_auth(&address)
                .inspect_err(|e| {
                    tracing::error!("failed to create client for {} \n {:?}", address.clone(), e)
                })
                .ok()?;

            Some((Fleek::get_epoch_info(&client).await.ok()?, address))
        };
        futs.push(fut);
    }

    let results: Vec<(EpochInfo, String)> = futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
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
) -> Result<()> {
    for _ in 0..max_tries {
        let node_info = query_node_info(public_key, genesis_committee)
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
