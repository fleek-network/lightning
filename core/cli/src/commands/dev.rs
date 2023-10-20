use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::types::CompressionAlgorithm;
use lightning_interfaces::{
    BlockStoreInterface,
    BlockStoreServerInterface,
    ConfigProviderInterface,
    IncrementalPutInterface,
};
use lightning_node::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::error;

use crate::args::DevSubCmd;

pub async fn exec<C>(cmd: DevSubCmd, config_path: ResolvedPathBuf) -> Result<()>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    match cmd {
        DevSubCmd::InitOnly => init::<C>(config_path).await,
        DevSubCmd::ShowOrder => show_order::<C>().await,
        DevSubCmd::DepGraph => dep_graph::<C>().await,
        DevSubCmd::Store { input } => store::<C>(input, config_path).await,
        DevSubCmd::Fetch { remote, hash } => fetch::<C>(config_path, hash, remote).await,
    }
}

async fn init<C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    Node::<C>::init(config)
        .map_err(|e| anyhow::anyhow!("Node Initialization failed: {e:?}"))
        .context("Could not start the node.")?;
    Ok(())
}

async fn show_order<C: Collection>() -> Result<()> {
    let graph = C::build_graph();
    let sorted = graph
        .sort()
        .map_err(|e| anyhow!("Sort graph error: {e:?}"))?;
    for (i, tag) in sorted.iter().enumerate() {
        println!(
            "{:0width$}  {tag}\n      = {ty}",
            i + 1,
            width = 2,
            tag = tag.trait_name(),
            ty = tag.type_name()
        );
    }
    Ok(())
}

async fn dep_graph<C: Collection>() -> Result<()> {
    let graph = C::build_graph();
    let mermaid = graph.mermaid("Lightning Dependency Graph");
    println!("{mermaid}");
    Ok(())
}

async fn store<C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>>(
    input: Vec<PathBuf>,
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    let store = <C::BlockStoreInterface as BlockStoreInterface<C>>::init(
        config.get::<C::BlockStoreInterface>(),
    )
    .context("Could not init blockstore")?;

    let mut block = vec![0u8; 256 * 1025];

    'outer: for path in &input {
        let Ok(mut file) = File::open(path) else {
            error!("Could not open the file {path:?}");
            continue;
        };

        let mut putter = store.put(None);

        loop {
            let Ok(size) = file.read(&mut block) else {
                error!("read error");
                break 'outer;
            };

            if size == 0 {
                break;
            }

            putter
                .write(&block[0..size], CompressionAlgorithm::Uncompressed)
                .unwrap();
        }

        match putter.finalize().await {
            Ok(hash) => {
                println!("{:x}\t{path:?}", ByteBuf(&hash));
            },
            Err(e) => {
                error!("Failed: {e}");
            },
        }
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

    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    let node = Node::<C>::init(config)
        .map_err(|e| anyhow::anyhow!("Node Initialization failed: {e:?}"))
        .context("Could not start the node.")?;

    tracing::info!("Starting the node.");
    node.start().await;

    let blockstore = node
        .container
        .get::<C::BlockStoreServerInterface>(infusion::tag!(C::BlockStoreServerInterface));

    let socket = blockstore.get_socket();

    tracing::info!("Downloading {hash_string} from peer {peer}");
    let mut result = socket
        .run(lightning_types::ServerRequest { hash, peer })
        .await
        .expect("Failed to send task.");

    tracing::info!("Submitted the request to the socket and waiting for response.");
    match result.recv().await {
        Ok(Ok(())) => {
            tracing::info!("Got OK throught the socket. Download complete.");
        },
        Ok(Err(e)) => {
            tracing::error!("Got an error from the socket. Download incomplete. {e:?}");
            return Err(e.into());
        },
        Err(e) => {
            tracing::error!("Failed to receive response from socket: {e:?}");
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
