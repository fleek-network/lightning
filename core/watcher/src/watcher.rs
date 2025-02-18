use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use futures::StreamExt;
use lightning_interfaces::schema::task_broker::{TaskRequest, TaskResponse, TaskScope};
use lightning_interfaces::types::{ExecuteTransactionRequest, JobInfo, JobStatus};
use lightning_interfaces::{
    c,
    NodeComponents,
    NotifierInterface,
    SignerSubmitTxSocket,
    TaskError,
    WatcherInterface,
};
use lightning_types::UpdateMethod;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task::{JoinError, JoinHandle};
use tokio::time::{Instant, Interval};

pub struct Watcher<C: NodeComponents> {
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    task_broker: c!(C::TaskBrokerInterface),
    signer: SignerSubmitTxSocket,
    pk: NodePublicKey,
    notifier: C::NotifierInterface,
    tasks:
        FuturesUnordered<JoinHandle<([u8; 32], Vec<std::result::Result<TaskResponse, TaskError>>)>>,
    interval_counter: u32,
    interval: Interval,
}

impl<C: NodeComponents> Watcher<C> {
    pub fn new(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        task_broker: c!(C::TaskBrokerInterface),
        signer: SignerSubmitTxSocket,
        notifier: C::NotifierInterface,
        pk: NodePublicKey,
    ) -> Self {
        let duration = app_query.get_time_interval();
        let interval = tokio::time::interval(Duration::from_millis(duration));

        Self {
            app_query,
            signer,
            pk,
            task_broker,
            notifier,
            tasks: FuturesUnordered::new(),
            interval,
        }
    }

    fn perform_scheduled_work(&self) {
        if let Some(index) = self.app_query.pubkey_to_index(&self.pk) {
            if let Some(jobs) = self.app_query.get_jobs_for_node(&index) {
                for job in jobs {
                    let JobInfo {
                        service,
                        arguments,
                        frequency,
                        ..
                    } = job.info;

                    if self.interval_counter % frequency != 0 {
                        continue;
                    }

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

    /// Increments the counter.
    fn update_counter(&mut self) {
        self.interval_counter += 1;
    }

    /// Resets the counter and recomputes the interval.
    fn reset(&mut self) {
        self.interval_counter = 0;
        let duration = self.app_query.get_time_interval();
        self.interval = tokio::time::interval(Duration::from_millis(duration));
    }

    fn tick(&mut self) -> Instant {
        self.interval.as_mut().unwrap().tick()
    }

    pub async fn start(mut self) {
        self.reset();

        let mut epoch_changed_sub = self.notifier.subscribe_epoch_changed();

        loop {
            tokio::select! {
                biased;
                _ = epoch_changed_sub.recv() => {
                    self.reset();
                }
                _ = self.interval.tick() => {
                    self.update_counter();
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

        tx
    }
}

impl<C: NodeComponents> WatcherInterface<C> for Watcher<C> {}
