use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use futures::StreamExt;
use lightning_interfaces::schema::task_broker::{TaskRequest, TaskResponse, TaskScope};
use lightning_interfaces::types::{ExecuteTransactionRequest, JobInfo, JobStatus};
use lightning_interfaces::{c, NodeComponents, SignerSubmitTxSocket, TaskError, WatcherInterface};
use lightning_types::UpdateMethod;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task::{JoinError, JoinHandle};

pub struct Watcher<C: NodeComponents> {
    app_query: QueryRunner,
    task_broker: c!(C::TaskBrokerInterface),
    signer: SignerSubmitTxSocket,
    pk: NodePublicKey,
    tasks:
        FuturesUnordered<JoinHandle<([u8; 32], Vec<std::result::Result<TaskResponse, TaskError>>)>>,
}

impl<C: NodeComponents> Watcher<C> {
    pub fn new(
        app_query: QueryRunner,
        task_broker: c!(C::TaskBrokerInterface),
        signer: SignerSubmitTxSocket,
        pk: NodePublicKey,
    ) -> Self {
        Self {
            app_query,
            signer,
            pk,
            task_broker,
            tasks: FuturesUnordered::new(),
        }
    }

    fn perform_scheduled_work(&self) {
        if let Some(index) = self.app_query.pubkey_to_index(&self.pk) {
            if let Some(jobs) = self.app_query.get_jobs_for_node(&index) {
                for job in jobs {
                    let JobInfo {
                        service, arguments, ..
                    } = job.info;

                    // Arguments should have this information.
                    /*
                    pub struct Request {
                        /// Origin to use
                        pub origin: Origin,
                        /// URI For the origin
                        /// - for blake3 should be hex encoded bytes
                        /// - for ipfs should be cid string
                        pub uri: String,
                        /// Optional path to provide as the window location,
                        /// including query parameters and the fragment.
                        pub path: Option<String>,
                        /// Parameter to pass to the script's main function.
                        /// For http oriented functions, an object can be passed
                        /// here to simulate the http object with the fields for
                        /// method, headers, query params, and the url fragment
                        pub param: Option<serde_json::Value>,
                    }
                     */
                    let job_hash = job.hash;
                    let request = TaskRequest {
                        service,
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        payload: arguments.into(),
                    };
                    let task_broker = self.task_broker.clone();
                    self.tasks.push(tokio::spawn(async move {
                        (
                            job_hash,
                            task_broker.run(1, TaskScope::Local, request).await,
                        )
                    }));
                }
            }
        }
    }

    async fn process_finished_jobs(
        &self,
        job_hash: [u8; 32],
        job_result: Vec<std::result::Result<TaskResponse, TaskError>>,
    ) {
        // This is only an estimate of when we were noticed that the job finished.
        let last_run = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut succeeded = true;
        let mut message = None;
        for result in job_result {
            // Todo: How do we know if the overall task finished successfully?
            // For now assume that if fails if there is at least one error.
            if let Err(e) = result {
                succeeded = false;
                message = Some(e.to_string());
            }
        }

        if let Err(e) = self
            .signer
            .run(ExecuteTransactionRequest {
                method: UpdateMethod::JobUpdates {
                    updates: vec![JobStatus {
                        success: succeeded,
                        message,
                        last_run,
                    }],
                },
                options: None,
            })
            .await
        {
            tracing::error!("failed to send a job update")
        }
    }

    pub fn spawn(mut self) -> Sender<()> {
        let (tx, mut rx) = mpsc::channel(1);

        tokio::spawn(async move {
            // We assume that the duration unit is milliseconds.
            let duration = self.app_query.get_time_interval();
            let mut interval = tokio::time::interval(Duration::from_millis(duration));
            loop {
                tokio::select! {
                    biased;
                    shutdown = rx.recv() => {
                        if shutdown.is_none() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        self.perform_scheduled_work();
                    }
                    Some(next) = self.tasks.next() => {
                        match next {
                            Ok((hash, result)) => {
                                self.process_finished_jobs(hash, result);
                            }
                            Err(e) => {
                                tracing::warn!("scheduled job panicked!")
                            }
                        }
                    }
                }
            }
        });

        tx
    }
}

impl<C: NodeComponents> WatcherInterface<C> for Watcher<C> {}
