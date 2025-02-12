use std::sync::Arc;
use std::time::Duration;

use affair::AsyncWorker;
use lightning_interfaces::prelude::*;
use lightning_interfaces::SignerError;
use lightning_utils::poll::{poll_until, PollUntilError};
use lightning_utils::transaction::TransactionClient;
use tokio::sync::Mutex;
use types::{ExecuteTransactionRequest, ExecuteTransactionResponse};

/// An affair worker that executes incoming transactions using a transaction client.
#[derive(Clone)]
pub struct SignerWorker<C: NodeComponents> {
    client: Arc<Mutex<Option<TransactionClient<C>>>>,
    shutdown: ShutdownWaiter,
}

impl<C: NodeComponents> SignerWorker<C> {
    pub fn new(shutdown: ShutdownWaiter) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            shutdown,
        }
    }

    pub async fn start(&self, client: TransactionClient<C>) {
        // Lock mutex to update client within existing Arc/Mutex.
        // This ensures all clones of self.client see the update, as replacing the Arc would
        // disconnect existing clones.
        let mut client_lock = self.client.lock().await;
        *client_lock = Some(client);
    }
}

impl<C: NodeComponents> AsyncWorker for SignerWorker<C> {
    type Request = ExecuteTransactionRequest;
    type Response = Result<ExecuteTransactionResponse, SignerError>;

    async fn handle(&mut self, request: Self::Request) -> Self::Response {
        tracing::info!("handling signer request: {:?}", request);

        self.shutdown
            .run_until_shutdown(async {
                // If the signer hasn't started yet, block the request for a short period, waiting
                // for it to be ready. This should only ever happen during node
                // startup if an internal node transaction is submitted before the
                // signer starts up.
                let _ = poll_until(
                    || async {
                        self.client
                            .lock()
                            .await
                            .is_some()
                            .then_some(())
                            .ok_or(PollUntilError::ConditionNotSatisfied)
                    },
                    Duration::from_secs(5),
                    Duration::from_millis(100),
                )
                .await;

                // Check that the client is ready, and return an error if not.
                let client = self.client.lock().await;
                let Some(client) = client.as_ref() else {
                    tracing::info!("signer not ready");
                    return Err(SignerError::NotReady);
                };

                // Execute the transaction via the client.
                tracing::info!("executing transaction: {:?}", request);
                let resp = client
                    .execute_transaction(request.method, request.options)
                    .await?;
                tracing::info!("successfully executed transaction");

                Ok(resp)
            })
            .await
            .ok_or(SignerError::ShuttingDown)?
    }
}
