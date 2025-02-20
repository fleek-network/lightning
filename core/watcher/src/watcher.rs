use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lightning_interfaces::prelude::{BuildGraph, *};
use lightning_interfaces::schema::task_broker::{TaskRequest, TaskResponse, TaskScope};
use lightning_interfaces::types::{ExecuteTransactionRequest, JobInfo, JobStatus};
use lightning_interfaces::{
    c,
    fdi,
    ApplicationInterface,
    NodeComponents,
    NotifierInterface,
    ShutdownWaiter,
    SignerSubmitTxSocket,
    Subscriber,
    SyncQueryRunnerInterface,
    TaskBrokerInterface,
    TaskError,
    WatcherInterface,
};
use lightning_types::UpdateMethod;
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Interval;

#[derive(Clone)]
pub struct Watcher<C: NodeComponents> {
    inner: Arc<Mutex<Option<InnerWatcher<C>>>>,
}

impl<C: NodeComponents> Watcher<C> {
    pub fn new(
        task_broker: &C::TaskBrokerInterface,
        signer: &C::SignerInterface,
        notifier: &C::NotifierInterface,
        keystore: &C::KeystoreInterface,
        fdi::Cloned(app_query): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Result<Self> {
        let duration = app_query.get_time_interval();
        let interval = tokio::time::interval(Duration::from_millis(duration));
        let pk = keystore.get_ed25519_pk();

        Ok(Self {
            inner: Arc::new(Mutex::new(Some(InnerWatcher {
                app_query: app_query.clone(),
                signer: signer.get_socket(),
                pk,
                task_broker: task_broker.clone(),
                notifier: notifier.clone(),
                tasks: FuturesUnordered::new(),
                interval,
                interval_counter: 0,
            }))),
        })
    }

    pub async fn start(
        this: fdi::Ref<Self>,
        fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>,
    ) -> Result<()> {
        let shutdown = shutdown.clone();
        let mut inner = this
            .inner
            .lock()
            .await
            .take()
            .expect("Watcher state to exist");
        spawn!(
            async move {
                inner.start(shutdown.clone()).await;
            },
            "WATCHER: spawn event loop"
        );

        Ok(())
    }
}

struct InnerWatcher<C: NodeComponents> {
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

impl<C: NodeComponents> InnerWatcher<C> {
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
                    updates: BTreeMap::from([(
                        job_hash,
                        JobStatus {
                            success: succeeded,
                            message,
                            last_run,
                        },
                    )]),
                },
                options: None,
            })
            .await
        {
            tracing::error!("failed to send a job update: {e:?}")
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

    pub async fn start(&mut self, shutdown: ShutdownWaiter) {
        let mut epoch_changed_sub = self.notifier.subscribe_epoch_changed();

        loop {
            tokio::select! {
                biased;
                _ = shutdown.wait_for_shutdown() => {
                    break;
                }
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
                            self.process_finished_jobs(hash, result).await;
                        }
                        Err(e) => {
                            tracing::warn!("scheduled job panicked!: {e:?}")
                        }
                    }
                }
            }
        }
    }
}

impl<C: NodeComponents> WatcherInterface<C> for Watcher<C> {}

impl<C: NodeComponents> BuildGraph for Watcher<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(
            Self::new.with_event_handler("start", Self::start.wrap_with_spawn_named("WATCHER")),
        )
    }
}
