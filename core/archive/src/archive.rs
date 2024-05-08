use std::sync::Arc;

use anyhow::{Context, Result};
use ethers::types::BlockNumber;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Block,
    BlockExecutionResponse,
    BlockReceipt,
    TransactionReceipt,
    TransactionRequest,
};
use resolved_pathbuf::ResolvedPathBuf;
use rocksdb::{Options, DB};
use tokio::pin;

use crate::config::Config;

// Column families
const BLKHASH_TO_BLKNUM: &str = "blkhash_to_blknum";
const BLKNUM_TO_BLK: &str = "blknum_to_blk";
const TXHASH_TO_TXRCT: &str = "txhash_to_txrct";
const MISC: &str = "misc";

// Special keys
const LATEST: &str = "latest";
const EARLIEST: &str = "earliest";

pub struct Archive<C: Collection> {
    inner: Option<Arc<ArchiveInner<C>>>,
}

struct ArchiveInner<C: Collection> {
    db: DB,
    blockstore: c!(C::BlockstoreInterface),
    /// Handles the rocks db storage for each epoch
    historical_state_dir: ResolvedPathBuf,
}

impl<C: Collection> BuildGraph for Archive<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new()
            .with_infallible(Self::new.on("start", insertion_task::<C>.spawn()))
    }
}

impl<C: Collection> Archive<C> {
    pub fn new(
        config_provider: &C::ConfigProviderInterface,
        blockstore: &C::BlockstoreInterface,
    ) -> Self {
        let config = config_provider.get::<Self>();

        if !config.is_archive {
            return Self { inner: None };
        }

        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        db_options.create_missing_column_families(true);

        let cf = vec![BLKHASH_TO_BLKNUM, BLKNUM_TO_BLK, TXHASH_TO_TXRCT, MISC];
        let db =
            DB::open_cf(&db_options, &config.store_path, cf).expect("Failed to create archive db");

        let historical_state_dir: ResolvedPathBuf = config
            .store_path
            .join("historical")
            .try_into()
            .expect("resolved historical dir");

        if !historical_state_dir.is_dir() {
            std::fs::create_dir_all(&historical_state_dir)
                .expect("Failed to create historical dir");
        }

        let inner = ArchiveInner::<C>::new(db, historical_state_dir, blockstore.clone());

        Self {
            inner: Some(Arc::new(inner)),
        }
    }
}

impl<C: Collection> ArchiveInterface<C> for Archive<C> {
    fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    async fn get_block_by_hash(&self, hash: [u8; 32]) -> Option<BlockReceipt> {
        self.inner.as_ref().and_then(|inner| {
            inner
                .get_block_by_hash(&hash)
                .ok()
                .flatten()
                .map(|info| info.receipt)
        })
    }

    async fn get_block_by_number(&self, number: BlockNumber) -> Option<BlockReceipt> {
        self.inner.as_ref().and_then(|inner| {
            inner
                .get_block_by_block_number(&number)
                .ok()
                .flatten()
                .map(|info| info.receipt)
        })
    }

    async fn get_transaction_receipt(&self, hash: [u8; 32]) -> Option<TransactionReceipt> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.get_transaction_receipt(&hash).ok().flatten())
    }

    async fn get_transaction(&self, hash: [u8; 32]) -> Option<TransactionRequest> {
        self.inner.as_ref().and_then(|inner| {
            inner
                .get_transaction_receipt(&hash)
                .ok()
                .flatten()
                .and_then(|receipt| {
                    inner
                        .get_block_by_hash(&receipt.block_hash)
                        .ok()
                        .flatten()
                        .and_then(|info| {
                            info.block
                                .transactions
                                .get(receipt.transaction_index as usize)
                                .cloned()
                        })
                })
        })
    }

    async fn get_historical_epoch_state(
        &self,
        epoch: u64,
    ) -> Option<c![C::ApplicationInterface::SyncExecutor]> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.get_historical_query_runner(epoch).ok())
    }
}

impl<C: Collection> Clone for Archive<C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Main logic to handle archive requests
impl<C: Collection> ArchiveInner<C> {
    fn new(
        db: DB,
        historical_state_dir: ResolvedPathBuf,
        blockstore: c!(C::BlockstoreInterface),
    ) -> Self {
        Self {
            db,
            historical_state_dir,
            blockstore,
        }
    }

    fn get_historical_query_runner(
        &self,
        epoch: u64,
    ) -> Result<c!(C::ApplicationInterface::SyncExecutor)> {
        let path = self.historical_state_dir.join(epoch.to_string());
        tracing::trace!(target: "archive", "Getting historical epoch state from {:?}", path);
        let db = <c!(C::ApplicationInterface::SyncExecutor)>::atomo_from_path(path)?;
        let query_runner = <c!(C::ApplicationInterface::SyncExecutor)>::new(db);
        Ok(query_runner)
    }

    fn get_block_by_hash(&self, blk_hash: &[u8; 32]) -> Result<Option<BlockInfo>> {
        let blkhash_cf = self
            .db
            .cf_handle(BLKHASH_TO_BLKNUM)
            .context("Column family `blkhash_to_blknum` not found in db")?;
        let Some(blk_num) = self.db.get_cf(&blkhash_cf, blk_hash)? else {
            return Ok(None);
        };
        self.get_block_by_num(&blk_num)
    }

    // Gets the block for the BlockNumber type from ethers
    fn get_block_by_block_number(&self, blk_num: &BlockNumber) -> Result<Option<BlockInfo>> {
        let get_block_by_key = |key| {
            let misc_cf = self
                .db
                .cf_handle(MISC)
                .context("Column family `misc` not found in db")?;
            let Some(blk_num) = self.db.get_cf(&misc_cf, key)? else {
                return Ok(None);
            };
            self.get_block_by_num(&blk_num)
        };
        match blk_num {
            BlockNumber::Latest | BlockNumber::Finalized | BlockNumber::Safe => {
                get_block_by_key(LATEST)
            },
            BlockNumber::Earliest => get_block_by_key(EARLIEST),
            BlockNumber::Number(num) => {
                let mut blk_num = vec![0; 8];
                num.to_little_endian(&mut blk_num);
                self.get_block_by_num(&blk_num)
            },
            BlockNumber::Pending => Ok(None),
        }
    }

    // Gets the block for an actual block number (integer)
    fn get_block_by_num(&self, blk_num: &[u8]) -> Result<Option<BlockInfo>> {
        let blknum_cf = self
            .db
            .cf_handle(BLKNUM_TO_BLK)
            .context("Column family `blknum_to_blk` not found in db")?;
        let Some(blk_info_bytes) = self.db.get_cf(&blknum_cf, blk_num)? else {
            return Ok(None);
        };
        let blk_info: BlockInfo = blk_info_bytes.try_into()?;
        Ok(Some(blk_info))
    }

    fn get_transaction_receipt(&self, tx_hash: &[u8; 32]) -> Result<Option<TransactionReceipt>> {
        let txhash_cf = self
            .db
            .cf_handle(TXHASH_TO_TXRCT)
            .context("Column family `txhash_to_txrct` not found in db")?;
        match self.db.get_cf(&txhash_cf, tx_hash)? {
            Some(receipt_bytes) => {
                let receipt = bincode::deserialize(&receipt_bytes)?;
                Ok(Some(receipt))
            },
            None => Ok(None),
        }
    }

    async fn handle_epoch(&self, epoch: u64, hash: [u8; 32]) -> Result<()> {
        let path = self.historical_state_dir.join(epoch.to_string());

        // read the checkpoint from the blockstore, at this point application/env::run() has already
        // written this to the blockstore
        tracing::trace!(target: "archive", "Reading checkpoint from blockstore for epoch {}", epoch);
        let checkpoint = match self.blockstore.read_all_to_vec(&hash).await {
            Some(checkpoint) => checkpoint,
            None => {
                return Err(anyhow::anyhow!(
                    "Could not find checkpoint in blockstore for epoch, this is a bug"
                ));
            },
        };

        // create the query runner, this will write the checkpoint to the historical state dir
        let db = <c!(C::ApplicationInterface::SyncExecutor)>::atomo_from_checkpoint(
            path,
            hash,
            &checkpoint,
        )?;

        // we dont actullay need to do anything with the query runner, so we ignore it explicity
        let _ = <c!(C::ApplicationInterface::SyncExecutor)>::new(db);

        Ok(())
    }

    fn handle_block(&self, block: Block, response: BlockExecutionResponse) -> Result<()> {
        let (blk_receipt, txn_receipts) = response.to_receipts();
        let blk_info = BlockInfo {
            block,
            receipt: blk_receipt,
        };

        // Store BlockNum => BlockInfo
        let blknum_cf = self
            .db
            .cf_handle(BLKNUM_TO_BLK)
            .context("Column family `blknum_to_blk` not found in db")?;
        let blk_num = blk_info.receipt.block_number.to_le_bytes();
        let blk_info_bytes: Vec<u8> = (&blk_info).try_into()?;
        self.db.put_cf(&blknum_cf, blk_num, blk_info_bytes)?;

        let misc_cf = self
            .db
            .cf_handle(MISC)
            .context("Column family `misc` not found in db")?;

        // Store the first block, if we haven't already
        if self.db.get_cf(&misc_cf, EARLIEST)?.is_none() {
            // Once we prune old blocks from the archiver, we have to store the actual block info
            // here.
            self.db.put_cf(&misc_cf, EARLIEST, blk_num)?;
        }

        // Store the latest block number
        self.db.put_cf(&misc_cf, LATEST, blk_num)?;

        // Store BlockHash => BlockNum
        let blkhash_cf = self
            .db
            .cf_handle(BLKHASH_TO_BLKNUM)
            .context("Column family `blkhash_to_blknum` not found in db")?;
        self.db
            .put_cf(&blkhash_cf, blk_info.receipt.block_hash, blk_num)?;

        // Store TxHash => TxReceipt for each tx in the block
        let txhash_cf = self
            .db
            .cf_handle(TXHASH_TO_TXRCT)
            .context("Column family `txhash_to_txrct` not found in db")?;
        for txn_receipt in txn_receipts {
            let txn_receipt_bytes = bincode::serialize(&txn_receipt)?;
            self.db
                .put_cf(&txhash_cf, txn_receipt.transaction_hash, txn_receipt_bytes)?;
        }
        Ok(())
    }
}

/// The main loop that listens to notifier events and inserts the data into the db.
async fn insertion_task<C: Collection>(
    fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
    fdi::Cloned(archive): fdi::Cloned<Archive<C>>,
) {
    let Some(inner) = archive.inner else {
        return;
    };

    let mut block_executed_sub = notifier.subscribe_block_executed();
    let mut epoch_changed_sub = notifier.subscribe_epoch_changed();
    let shutdown_fut = waiter.wait_for_shutdown();
    pin!(shutdown_fut);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_fut => {
                break;
            }
            Some(n) = epoch_changed_sub.recv() => {
                let _ = inner.handle_epoch(n.current_epoch, n.last_epoch_hash).await;
            },
            Some(n) = block_executed_sub.recv() => {
                let _ = inner.handle_block(n.block, n.response);
            },
            else => {
                break;
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BlockInfo {
    pub block: Block,
    pub receipt: BlockReceipt,
}

impl TryFrom<&BlockInfo> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: &BlockInfo) -> std::result::Result<Self, Self::Error> {
        let mut bytes = Vec::new();
        let block_bytes: Vec<u8> = (&value.block).try_into()?;
        let block_len = block_bytes.len() as u64;
        bytes.extend_from_slice(&block_len.to_le_bytes());
        bytes.extend_from_slice(&block_bytes);
        let receipt_bytes = bincode::serialize(&value.receipt)?;
        let receipt_len = receipt_bytes.len() as u64;
        bytes.extend_from_slice(&receipt_len.to_le_bytes());
        bytes.extend_from_slice(&receipt_bytes);
        Ok(bytes)
    }
}

impl TryFrom<Vec<u8>> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        let block_len_bytes: [u8; 8] = value.get(0..8).context("Out of bounds")?.try_into()?;
        let block_len = u64::from_le_bytes(block_len_bytes) as usize;
        let block_bytes = value.get(8..block_len + 8).context("Out of bounds")?;
        let block = Block::try_from(block_bytes.to_vec())?;

        let receipt_len_bytes: [u8; 8] = value
            .get(block_len + 8..block_len + 16)
            .context("Out of bounds")?
            .try_into()?;
        let receipt_len = u64::from_le_bytes(receipt_len_bytes) as usize;
        let receipt_bytes = value
            .get(block_len + 16..block_len + receipt_len + 16)
            .context("Out of bounds")?;
        let receipt: BlockReceipt = bincode::deserialize(receipt_bytes)?;
        Ok(BlockInfo { block, receipt })
    }
}

impl<C: Collection> ConfigConsumer for Archive<C> {
    const KEY: &'static str = "archive";

    type Config = Config;
}
