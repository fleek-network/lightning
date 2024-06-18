use std::path::PathBuf;

use anyhow::{Context, Result};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Blake3Hash, NodePorts};
use lightning_utils::config::TomlConfigProvider;
use lightning_utils::rpc::rpc_request;
use reqwest::Client;
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::json;

use crate::args::DevSubCmd;

pub async fn exec<C>(cmd: DevSubCmd, config_path: ResolvedPathBuf) -> Result<()>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    match cmd {
        DevSubCmd::DepGraph => dep_graph::<C>().await,
        DevSubCmd::Store { input } => store(input).await,
        DevSubCmd::Fetch { remote, hash } => fetch::<C>(config_path, hash, remote).await,
    }
}

async fn dep_graph<C: Collection>() -> Result<()> {
    let graph = C::build_graph();
    let mermaid = graph.viz("Lightning Dependency Graph");
    println!("{mermaid}");
    Ok(())
}

async fn store(input: Vec<PathBuf>) -> Result<()> {
    // Todo: is there a way to find the port for the locally running node?
    let ports = NodePorts::default();

    for path in &input {
        if let Some(path) = path.to_str() {
            let request = json!({
                "jsonrpc": "2.0",
                "method": "admin_store",
                "params": { "path": path },
                "id":1,
            })
            .to_string();
            let client = Client::new();
            let response = rpc_request::<Blake3Hash>(
                &client,
                format!("http://127.0.0.1:{}/admin", ports.rpc),
                request,
                None,
            )
            .await?;

            println!("{:x}\t{path:?}", ByteBuf(&response.result));
        } else {
            println!("invalid unicode in {path:?}")
        };
    }
    Ok(())
}

async fn fetch<C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>>(
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
