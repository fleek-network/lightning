use tokio::sync::oneshot;

use crate::{TransactionReceipt, UpdateMethod};

#[derive(Debug)]
pub struct ExecuteTransaction {
    pub method: UpdateMethod,
    pub receipt_tx: Option<oneshot::Sender<TransactionReceipt>>,
}

impl From<UpdateMethod> for ExecuteTransaction {
    fn from(value: UpdateMethod) -> Self {
        Self {
            method: value,
            receipt_tx: None,
        }
    }
}
