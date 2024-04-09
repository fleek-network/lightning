use std::sync::Arc;

use derive_deref::Deref;
use ebpf_service::client::EbpfSvcClient;
use tokio::sync::Mutex;

#[derive(Clone, Default, Deref)]
pub struct Client(Arc<Mutex<EbpfSvcClient>>);
