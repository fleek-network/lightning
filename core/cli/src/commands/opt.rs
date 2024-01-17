use std::fs::read_to_string;
use std::io::stdin;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

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
use lightning_signer::Signer;
use lightning_types::{
    ChainId,
    EpochInfo,
    NodeInfo,
    Participation,
    TransactionRequest,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_utils::rpc::rpc_request;
use reqwest::Client;
use resolved_pathbuf::ResolvedPathBuf;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::args::OptSubCmd;

pub async fn exec<C: Collection<SignerInterface = Signer<C>>>(
    cmd: OptSubCmd,
    config_path: ResolvedPathBuf,
) -> Result<()> {
    match cmd {
        OptSubCmd::In => opt_in::<C>(config_path).await,
        OptSubCmd::Out => opt_out::<C>(config_path).await,
        OptSubCmd::Status => status::<C>(config_path).await,
    }
}

async fn opt_in<C: Collection<SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    println!(
        "After sending the OptIn transaction, you are expected to start your node immediately. Is your node built and ready to start? (y/N)"
    );
    get_user_confirmation();

    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();

    let genesis_committee =
        get_genesis_committee().context("Failed to load genesis committee info.")?;

    let node_info =
        genesis_committee_rpc::<Option<NodeInfo>>(&genesis_committee, get_node_info(public_key))
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

    let chain_id = get_chain_id().context("Failed to load Chain ID from genesis.")?;

    let tx = create_update_request(
        UpdateMethod::OptIn {},
        secret_key,
        node_info.nonce + 1,
        chain_id,
    );

    genesis_committee_rpc::<()>(&genesis_committee, send_transaction(tx))
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

async fn opt_out<C: Collection<SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    println!(
        "Even after sending the OptOut transaction, your node is expected to stay online until the end of the current epoch. Do you want to continue? (y/N)"
    );
    get_user_confirmation();

    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();

    let genesis_committee =
        get_genesis_committee().context("Failed to load genesis committee info.")?;

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

    let chain_id = get_chain_id().context("Failed to load Chain ID from genesis.")?;

    let tx = create_update_request(
        UpdateMethod::OptOut {},
        secret_key,
        node_info.nonce + 1,
        chain_id,
    );

    genesis_committee_rpc::<()>(&genesis_committee, send_transaction(tx))
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

async fn status<C: Collection<SignerInterface = Signer<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = Arc::new(TomlConfigProvider::<C>::load_or_write_config(config_path).await?);
    let secret_key = load_secret_key(config.clone())?;
    let public_key = secret_key.to_pk();

    let genesis_committee =
        get_genesis_committee().context("Failed to load genesis committee info.")?;

    let node_info =
        genesis_committee_rpc::<Option<NodeInfo>>(&genesis_committee, get_node_info(public_key))
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
            println!(
                "Your node is opted out. You can shutdown your node once the current epoch ends in {}",
                get_timestamp(delta)
            );
            std::process::exit(212)
        },
    }
}

async fn get_epoch_end_delta(genesis_committee: &Vec<NodeInfo>) -> Result<Duration> {
    let epoch_info = genesis_committee_rpc::<EpochInfo>(genesis_committee, get_epoch_info())
        .await
        .context("Failed to get node info from genesis committee")?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let delta = (epoch_info.epoch_end).saturating_sub(now);
    Ok(Duration::from_millis(delta))
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

fn get_epoch_info() -> String {
    json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch_info",
        "params":[],
        "id":1,
    })
    .to_string()
}

fn get_chain_id() -> Result<ChainId> {
    let genesis = Genesis::load()?;
    Ok(genesis.chain_id)
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
        if let Ok(res) = rpc_request::<T>(
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
