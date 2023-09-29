use affair::Socket;
use infusion::c;
use lightning_types::Blake3Hash;
use tokio::sync::mpsc::Receiver;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    BlockStoreServerInterface,
    Notification,
    NotifierInterface,
    WithStartAndShutdown,
};

pub type CheckpointSocket = Socket<Blake3Hash, ()>;

#[infusion::service]
pub trait SyncronizerInterface<C: Collection>: WithStartAndShutdown + Sized {
    fn _init(
        app: ::ApplicationInterface,
        blockstore_server: ::BlockStoreServerInterface,
        notifier: ::NotifierInterface,
    ) {
        let sqr = app.sync_query();
        let (tx_epoch_change, rx_epoch_change) = tokio::sync::mpsc::channel(10);
        notifier.notify_on_new_epoch(tx_epoch_change);
        Self::init(sqr, blockstore_server.clone(), rx_epoch_change)
    }

    /// Create a syncronizer service for quickly syncronizing the node state with the chain
    fn init(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        blockstore_server: C::BlockStoreServerInterface,
        rx_epoch_change: Receiver<Notification>,
    ) -> anyhow::Result<Self>;

    /// Returns a socket that will send accross the blake3hash of the checkpoint
    /// Will send it after it has already downloaded from the blockstore server
    fn checkpoint_socket(&self) -> tokio::sync::oneshot::Receiver<Blake3Hash>;
}
