use std::sync::Arc;

use derive_deref::Deref;
use ebpf_service::client::IpcClient;
use tokio::sync::Mutex;

#[derive(Clone, Default, Deref)]
pub struct Client(Arc<Mutex<IpcClient>>);
