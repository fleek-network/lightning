use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use anyhow::Result;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::types::CompressionAlgorithm;
use lightning_interfaces::{BlockStoreInterface, ConfigProviderInterface, IncrementalPutInterface};
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::DevSubCmd;
use crate::config::TomlConfigProvider;

pub async fn exec<C>(cmd: DevSubCmd, config_path: ResolvedPathBuf) -> Result<()>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    match cmd {
        DevSubCmd::InitOnly => init::<C>(config_path).await,
        DevSubCmd::ShowOrder => show_order::<C>().await,
        DevSubCmd::DepGraph => dep_graph::<C>().await,
        DevSubCmd::Store { input } => store::<C>(input, config_path).await,
    }
}

async fn init<C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>>(
    config_path: ResolvedPathBuf,
) -> Result<()> {
    let config = TomlConfigProvider::<C>::load_or_write_config(config_path).await?;
    Node::<C>::init(config).map_err(|e| anyhow::anyhow!("Could not start the node: {e}"))?;
    Ok(())
}

async fn show_order<C: Collection>() -> Result<()> {
    let graph = C::build_graph();
    let sorted = graph.sort()?;
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
    .expect("Could not init blockstore");

    let mut block = vec![0u8; 256 * 1025];

    'outer: for path in &input {
        let Ok(mut file) = File::open(path) else {
                        log::error!("Could not open the file {path:?}");
                        continue;
                    };

        let mut putter = store.put(None);

        loop {
            let Ok(size) = file.read(&mut block) else {
                log::error!("read error");
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
                log::error!("Failed: {e}");
            },
        }
    }
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
