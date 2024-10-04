use std::path::PathBuf;

use anyhow::{Context, Result};
use lightning_interfaces::prelude::*;
use lightning_node::Node;
use lightning_rpc::interface::Admin;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::DevSubCmd;

pub async fn exec<C>(cmd: DevSubCmd, config_path: ResolvedPathBuf) -> Result<()>
where
    C: NodeComponents<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    match cmd {
        DevSubCmd::DepGraph => dep_graph::<C>().await,
        DevSubCmd::Store { input } => store::<C>(config_path, input).await,
        DevSubCmd::Fetch { remote, hash } => fetch::<C>(config_path, hash, remote).await,
        DevSubCmd::ResetStateTree => reset_state_tree::<C>(config_path).await,
    }
}

async fn dep_graph<C: NodeComponents>() -> Result<()> {
    let graph = C::build_graph();
    let mermaid = graph.viz("Lightning Dependency Graph");
    println!("{mermaid}");
    Ok(())
}

async fn reset_state_tree<C>(config_path: ResolvedPathBuf) -> Result<()>
where
    C: NodeComponents<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    let config = TomlConfigProvider::<C>::load(config_path)?;
    let app_config = config.get::<<C as NodeComponents>::ApplicationInterface>();

    C::ApplicationInterface::reset_state_tree_unsafe(&app_config)
}

async fn store<C>(config_path: ResolvedPathBuf, input: Vec<PathBuf>) -> Result<()>
where
    C: NodeComponents<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    let provider = TomlConfigProvider::<C>::load(config_path)?;
    let config = provider.get::<C::RpcInterface>();
    let port = <C::RpcInterface as RpcInterface<C>>::port(&config);
    let hmac_secret_path = <C::RpcInterface as RpcInterface<C>>::hmac_secret_dir(&config);

    // by default loads or creates if hmac_secret_path.is_none() the secret from ~/.lightning
    let url = format!("http://127.0.0.1:{}/admin", port);
    let secret = lightning_rpc::load_hmac_secret(hmac_secret_path)?;
    let client = lightning_rpc::RpcClient::new(&url, Some(&secret)).await?;

    for path in &input {
        if let Some(path) = path.to_str() {
            let response = Admin::store(&client, path.to_string()).await?;

            println!("{:x}\t{path:?}", ByteBuf(&response));
        } else {
            println!("invalid unicode in {path:?}")
        };
    }

    Ok(())
}

async fn fetch<C: NodeComponents<ConfigProviderInterface = TomlConfigProvider<C>>>(
    config_path: ResolvedPathBuf,
    hash_string: String,
    peer: u32,
) -> Result<()> {
    let hash: [u8; 32] = if hash_string.starts_with('[') {
        let pat: &[_] = &['[', ']'];
        let numbers: Vec<u8> = hash_string
            .trim_matches(pat)
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().expect("expected number"))
            .collect();

        if numbers.len() != 32 {
            anyhow::bail!("Failed to parse hash.");
        }

        let mut result = [0u8; 32];
        result.copy_from_slice(&numbers);
        result
    } else {
        fleek_blake3::Hash::from_hex(hash_string.as_bytes())
            .context("Invalid blake3 hash.")?
            .into()
    };
    let hash_string = fleek_blake3::Hash::from(hash).to_string();

    let config = TomlConfigProvider::<C>::load(config_path)?;
    let mut node = Node::<C>::init(config)
        .map_err(|e| anyhow::anyhow!("Node Initialization failed: {e:?}"))
        .context("Could not start the node.")?;

    tracing::info!("Starting the node.");
    node.start().await;

    let blockstore = node.provider.get::<C::BlockstoreServerInterface>();

    let socket = blockstore.get_socket();

    tracing::info!("Downloading {hash_string} from peer {peer}");
    let mut result = socket
        .run(lightning_interfaces::types::ServerRequest { hash, peer })
        .await
        .expect("Failed to send task.");

    tracing::info!("Submitted the request to the socket and waiting for response.");
    match result.recv().await {
        Ok(_) => {
            tracing::info!("Got OK throught the socket. Download complete.");
        },
        Err(e) => {
            tracing::error!("Got an error from the socket. Download incomplete. {e:?}");
            return Err(e.into());
        },
    }

    tracing::info!("Shutting the node down.");
    node.shutdown().await;

    Ok(())
}

struct ByteBuf<'a>(&'a [u8]);

impl<'a> std::fmt::LowerHex for ByteBuf<'a> {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        for byte in self.0 {
            fmtr.write_fmt(format_args!("{:02x}", byte))?;
        }
        Ok(())
    }
}
