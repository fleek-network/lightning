pub mod config;

use std::io::{Read, Write};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::RwLock;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use blake3_stream::{Encoder, VerifiedDecoder};
use blake3_tree::blake3::tree::HashTree;
use config::Config;
use lightning_interfaces::blockstore_server::BlockStoreServerInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm, NodeIndex};
use lightning_interfaces::{
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    IncrementalPutInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use log::error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use triomphe::Arc;

struct BlockStoreServer<C: Collection> {
    phantom: PhantomData<C>,
    config: Arc<Config>,
    blockstore: C::BlockStoreInterface,
    shutdown_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<()>>>>,
}

impl<C: Collection> Clone for BlockStoreServer<C> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
            config: self.config.clone(),
            blockstore: self.blockstore.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }
}

impl<C: Collection> ConfigConsumer for BlockStoreServer<C> {
    const KEY: &'static str = "blockserver";
    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for BlockStoreServer<C> {
    fn is_running(&self) -> bool {
        self.shutdown_tx.read().unwrap().is_some()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        let mut shutdown_entry = self.shutdown_tx.write().unwrap();
        if shutdown_entry.is_some() {
            return;
        }
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        *shutdown_entry = Some(tx);

        // spawn server task
        let address = self.config.address;
        let blockstore = self.blockstore.clone();
        tokio::spawn(async move {
            // bind to address
            let listener = TcpListener::bind(address)
                .await
                .expect("failed to bind to address");

            loop {
                select! {
                    Ok((socket, _)) = listener.accept() => {
                        if let Err(e) = handle_connection::<C>(blockstore.clone(), socket).await {
                            error!("error handling blockstore connection: {e}");
                        }
                    },
                    _ = rx.recv() => break,
                }
            }
        });
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        let sender = self.shutdown_tx.write().unwrap().take().unwrap();
        sender.send(()).await.unwrap();
    }
}

async fn handle_connection<C: Collection>(
    blockstore: C::BlockStoreInterface,
    mut socket: TcpStream,
) -> anyhow::Result<()> {
    let mut hash = [0u8; 32];
    socket.read_exact(&mut hash).await?;

    // fetch from the blockstore
    let Some(proof) = blockstore.get_tree(&hash).await else {
        return Err(anyhow!("failed to find block"));
    };

    // find out total content size
    let mut last_hash = [0; 32];
    let mut total = 0;
    for i in 0u32.. {
        let ii = (i * 2 - i.count_ones()) as usize;
        if ii >= proof.0.len() {
            break;
        }
        last_hash = proof.0[ii];
        total += 1;
    }
    let content_len = blockstore
        .get(total - 1, &last_hash, CompressionAlgoSet::default())
        .await
        .expect("last block not available")
        .content
        .len()
        // TODO: verify this is correct
        + (total as usize - 1) * 256 * 1024;

    let mut block_counter = 0u32;
    let mut block_hash = proof.0[0];

    // Setup stream encoder
    let std_socket = socket.into_std()?; // for now use std which supports io::Write
    let mut encoder = Encoder::new(
        std_socket,
        content_len,
        HashTree {
            hash: hash.into(),
            tree: proof.0.clone(),
        },
    )?;

    // Feed blocks to the stream
    while let Some(block) = blockstore
        .get(block_counter, &block_hash, CompressionAlgoSet::default())
        .await
    {
        encoder.write_all(&block.content)?;
        block_counter += 1;
        block_hash = proof.0[(block_counter * 2 - block_counter.count_ones()) as usize];
    }

    Ok(())
}

#[async_trait]
impl<C: Collection> BlockStoreServerInterface<C> for BlockStoreServer<C> {
    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        Ok(Self {
            phantom: PhantomData,
            config: config.into(),
            blockstore,
            shutdown_tx: Arc::new(RwLock::new(None)),
        })
    }

    fn extract_address<Q: SyncQueryRunnerInterface>(
        query_runner: Q,
        target: NodeIndex,
    ) -> Option<SocketAddr> {
        // Get node pk, info, and finally the address
        query_runner.index_to_pubkey(target).and_then(|pk| {
            query_runner
                .get_node_info(&pk)
                .map(|info| (info.domain, info.ports.blockstore).into())
        })
    }

    async fn request_download(&self, block_hash: Blake3Hash, target: SocketAddr) -> Result<()> {
        // Connect to the destination
        let mut socket = TcpStream::connect(target).await?;

        // Send request
        socket.write_all(&block_hash).await?;

        // Setup the decoder
        let std_socket = socket.into_std()?;
        let mut decoder = VerifiedDecoder::new(std_socket, block_hash);

        // TODO: Add a non-verified decoder to blake3-stream that yields proofs and chunks directly
        // without verification. We can let the blockstore verify for us, and avoid recomputing the
        // tree.
        let mut putter = self.blockstore.put(None);
        let mut buf = [0; 256 * 1024];
        loop {
            // Read a block of data, breaking if there is no more
            let len = decoder.read(&mut buf)?;
            if len == 0 {
                break;
            }

            // Feed the content into the blockstore
            putter.write(&buf[..len], CompressionAlgorithm::Uncompressed)?;
        }
        let hash = putter.finalize().await?;
        debug_assert_eq!(hash, block_hash);

        Ok(())
    }
}
