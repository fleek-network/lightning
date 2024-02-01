use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use affair::{Socket, Task};
use anyhow::{Context, Result};
use ethers::types::BlockNumber;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{
    Block,
    BlockExecutionResponse,
    BlockReceipt,
    TransactionReceipt,
};
use lightning_interfaces::{
    ApplicationInterface,
    ArchiveInterface,
    ArchiveRequest,
    ArchiveResponse,
    ArchiveSocket,
    BlockStoreInterface,
    ConfigConsumer,
    IndexRequest,
    IndexSocket,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use resolved_pathbuf::ResolvedPathBuf;
use rocksdb::{Options, DB};
use tokio::sync::{mpsc, Notify};
use tracing::error;

use crate::config::Config;

type ArchiveTask<C> = Task<ArchiveRequest, Result<ArchiveResponse<C>>>;
type IndexTask = Task<IndexRequest, Result<()>>;

// Column families
const BLKHASH_TO_BLKNUM: &str = "blkhash_to_blknum";
const BLKNUM_TO_BLK: &str = "blknum_to_blk";
const TXHASH_TO_TXRCT: &str = "txhash_to_txrct";
const EPOCH_TO_HASH: &str = "epoch";
const MISC: &str = "misc";

// Special keys
const LATEST: &str = "latest";
const EARLIEST: &str = "earliest";

pub struct Archive<C: Collection> {
    inner: Option<Arc<ArchiveInner<C>>>,
    /// This socket can be given out to other proccess to query the data that has been archived.
    /// Will be None if this node is not currently in archive mode
    archive_socket: Mutex<Option<ArchiveSocket<C>>>,
    /// This socket can be given out to other process to send things that should be archived,
    /// realisticlly only consensus should have this. Will return None if the node is not currently
    /// in archive mode
    index_socket: Mutex<Option<IndexSocket>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Option<Arc<Notify>>,
    _marker: PhantomData<C>,
}

impl<C: Collection> ArchiveInterface<C> for Archive<C> {
    fn init(
        config: Self::Config,
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore: C::BlockStoreInterface,
    ) -> anyhow::Result<Self> {
        if config.is_archive {
            let shutdown_notify = Arc::new(Notify::new());
            let (archive_socket, archive_rx) = Socket::raw_bounded(2048);
            let (index_socket, index_rx) = Socket::raw_bounded(2048);

            let mut db_options = Options::default();
            db_options.create_if_missing(true);
            db_options.create_missing_column_families(true);

            let cf = vec![BLKHASH_TO_BLKNUM, BLKNUM_TO_BLK, TXHASH_TO_TXRCT, MISC];
            let db = Arc::new(
                DB::open_cf(&db_options, &config.store_path, cf)
                    .expect("Failed to create archive db"),
            );

            let historical_state_dir: ResolvedPathBuf = config
                .store_path
                .join("historical")
                .try_into()
                .expect("resolved historical dir");

            if !historical_state_dir.is_dir() {
                std::fs::create_dir_all(&historical_state_dir)
                    .expect("Failed to create historical dir");
            }

            let inner = ArchiveInner::<C>::new(
                archive_rx,
                index_rx,
                db,
                shutdown_notify.clone(),
                historical_state_dir,
                blockstore,
            );
            Ok(Self {
                archive_socket: Mutex::new(Some(archive_socket)),
                index_socket: Mutex::new(Some(index_socket)),
                inner: Some(Arc::new(inner)),
                is_running: Arc::new(AtomicBool::new(false)),
                shutdown_notify: Some(shutdown_notify),
                _marker: PhantomData,
            })
        } else {
            Ok(Self {
                archive_socket: Mutex::new(None),
                index_socket: Mutex::new(None),
                inner: None,
                is_running: Arc::new(AtomicBool::new(false)),
                shutdown_notify: None,
                _marker: PhantomData,
            })
        }
    }

    fn archive_socket(&self) -> Option<ArchiveSocket<C>> {
        self.archive_socket.lock().unwrap().clone()
    }

    fn index_socket(&self) -> Option<IndexSocket> {
        self.index_socket.lock().unwrap().clone()
    }
}

impl<C: Collection> WithStartAndShutdown for Archive<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        if !self.is_running() {
            if let Some(inner) = self.inner.clone() {
                let is_running = self.is_running.clone();
                tokio::spawn(async move {
                    inner.start().await;
                    is_running.store(false, Ordering::Relaxed);
                });
                self.is_running.store(true, Ordering::Relaxed);
            }
        } else {
            error!("Can not start archive because it is already running");
        }
    }

    async fn shutdown(&self) {
        if let Some(shutdown_notify) = self.shutdown_notify.clone() {
            shutdown_notify.notify_one();
        }
    }
}

struct ArchiveInner<C: Collection> {
    archive_rx: Arc<Mutex<Option<mpsc::Receiver<ArchiveTask<C>>>>>,
    index_rx: Arc<Mutex<Option<mpsc::Receiver<IndexTask>>>>,
    db: Arc<DB>,
    shutdown_notify: Arc<Notify>,
    _marker: PhantomData<C>,
    blockstore: c!(C::BlockStoreInterface),
    /// Handles the rocks db storage for each epoch
    historical_state_dir: ResolvedPathBuf,
}

/// Main logic to handle archive requests
impl<C: Collection> ArchiveInner<C> {
    fn new(
        archive_rx: mpsc::Receiver<Task<ArchiveRequest, Result<ArchiveResponse<C>>>>,
        index_rx: mpsc::Receiver<Task<IndexRequest, Result<()>>>,
        db: Arc<DB>,
        shutdown_notify: Arc<Notify>,
        historical_state_dir: ResolvedPathBuf,
        blockstore: c!(C::BlockStoreInterface),
    ) -> Self {
        Self {
            archive_rx: Arc::new(Mutex::new(Some(archive_rx))),
            index_rx: Arc::new(Mutex::new(Some(index_rx))),
            db,
            shutdown_notify,
            historical_state_dir,
            blockstore,
            _marker: PhantomData,
        }
    }

    async fn start(&self) {
        let mut archive_rx = self.archive_rx.lock().unwrap().take().unwrap();
        let mut index_rx = self.index_rx.lock().unwrap().take().unwrap();

        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                Some(archive_task) = archive_rx.recv() => {
                    let res = self.handle_archive_request(&archive_task.request).await;
                    archive_task.respond(res);
                }
                Some(index_task) = index_rx.recv() => {
                    let res = self.handle_index_request(index_task.request.clone());
                    index_task.respond(res);
                }
            }
        }
        *self.archive_rx.lock().unwrap() = Some(archive_rx);
        *self.index_rx.lock().unwrap() = Some(index_rx);
    }

    async fn handle_archive_request(
        &self,
        archive_request: &ArchiveRequest,
    ) -> Result<ArchiveResponse<C>> {
        match archive_request {
            ArchiveRequest::GetBlockByHash(hash) => match self.get_block_by_hash(hash) {
                Ok(Some(block)) => Ok(ArchiveResponse::Block(block.receipt)),
                Ok(None) => Ok(ArchiveResponse::None),
                Err(e) => Err(e),
            },
            ArchiveRequest::GetBlockByNumber(blk_num) => {
                match self.get_block_by_block_number(blk_num) {
                    Ok(Some(block)) => Ok(ArchiveResponse::Block(block.receipt)),
                    Ok(None) => Ok(ArchiveResponse::None),
                    Err(e) => Err(e),
                }
            },
            ArchiveRequest::GetTransactionReceipt(tx_hash) => {
                match self.get_transaction_receipt(tx_hash) {
                    Ok(Some(recepit)) => Ok(ArchiveResponse::TransactionReceipt(recepit)),
                    Ok(None) => Ok(ArchiveResponse::None),
                    Err(e) => Err(e),
                }
            },
            ArchiveRequest::GetTransaction(tx_hash) => {
                let receipt = match self.get_transaction_receipt(tx_hash) {
                    Ok(Some(recepit)) => recepit,
                    Ok(None) => return Ok(ArchiveResponse::None),
                    Err(e) => return Err(e),
                };
                let blk_info = match self.get_block_by_hash(&receipt.block_hash) {
                    Ok(Some(block)) => block,
                    Ok(None) => return Ok(ArchiveResponse::None),
                    Err(e) => return Err(e),
                };
                match blk_info
                    .block
                    .transactions
                    .get(receipt.transaction_index as usize)
                {
                    Some(tx) => Ok(ArchiveResponse::Transaction(tx.clone())),
                    None => Ok(ArchiveResponse::None),
                }
            },
            ArchiveRequest::GetHistoricalEpochState(epoch) => {
                let path = self.historical_state_dir.join(epoch.to_string());

                if path.is_dir() {
                    // create the query runner
                    let db = <c!(C::ApplicationInterface::SyncExecutor)>::atomo_from_path(path)?;
                    let query_runner = <c!(C::ApplicationInterface::SyncExecutor)>::new(db);

                    // return the query runner
                    Ok(ArchiveResponse::HistoricalEpochState(query_runner))
                } else {
                    // if this epoch has happened we should have stored the hash
                    let hash: [u8; 32] = match self.db.get_cf(
                        // cursed ?
                        // this is !sync, so binding a variable here causes the compiler to
                        // complain
                        &self
                            .db
                            .cf_handle(EPOCH_TO_HASH)
                            .context("Column family `epoch` not found in db")?,
                        epoch.to_le_bytes(),
                    )? {
                        // will fail if the hash is longer than 32 bytes
                        Some(hash) => hash.try_into().map_err(|_| {
                            anyhow::anyhow!("Invalid hash in blockstore, this is a bug")
                        })?,
                        None => return Ok(ArchiveResponse::None),
                    };

                    // read the checkpoint from the blockstore
                    let checkpoint = match self.blockstore.read_all_to_vec(&hash).await {
                        Some(checkpoint) => checkpoint,
                        None => {
                            return Err(anyhow::anyhow!(
                                "Could not find checkpoint for epoch, this is a bug"
                            ));
                        },
                    };

                    // create the query runner
                    let db = <c!(C::ApplicationInterface::SyncExecutor)>::atomo_from_checkpoint(
                        path, hash, checkpoint,
                    )?;
                    let query_runner = <c!(C::ApplicationInterface::SyncExecutor)>::new(db);

                    // return the query runner
                    Ok(ArchiveResponse::HistoricalEpochState(query_runner))
                }
            },
        }
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
}

/// Incoming index requests from execution are sent here
impl<C: Collection> ArchiveInner<C> {
    fn handle_index_request(&self, index_request: IndexRequest) -> Result<()> {
        match index_request {
            IndexRequest::Block(block, response) => self.handle_block(block, response),
            IndexRequest::Epoch(epoch, digest) => self.handle_epoch(epoch, digest),
        }
    }

    fn handle_epoch(&self, epoch: u64, hash: [u8; 32]) -> Result<()> {
        let epoch_cf = self
            .db
            .cf_handle(EPOCH_TO_HASH)
            .context("Column family `epoch` not found in db")?;

        // Store Epoch => Hash
        self.db.put_cf(&epoch_cf, epoch.to_le_bytes(), hash)?;

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
