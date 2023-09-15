use std::ops::ControlFlow;

use lightning_interfaces::{ConnectionWorkStealer, ExecutorProviderInterface};
use serde::{Deserialize, Serialize};

use crate::state::StateRef;

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
#[serde(tag = "type")]
pub enum WorkerMode {
    AsyncWorker,
    BlockingWorker { use_tokio: bool },
}

pub fn attach_worker<P: ExecutorProviderInterface>(state: StateRef<P>, mode: WorkerMode) {
    let stealer = state.provider.get_work_stealer();

    match mode {
        WorkerMode::AsyncWorker => {
            tokio::task::spawn(non_blocking_worker(stealer, state));
        },
        WorkerMode::BlockingWorker { use_tokio } if use_tokio => {
            tokio::task::spawn_blocking(move || blocking_worker(stealer, state));
        },
        WorkerMode::BlockingWorker { .. } => {
            std::thread::spawn(move || blocking_worker(stealer, state));
        },
    }
}

fn blocking_worker<P: ExecutorProviderInterface>(mut stealer: P::Stealer, state: StateRef<P>) {
    while let Some(job) = stealer.next_blocking() {
        if state.shutdown.is_shutdown() {
            return;
        }

        state.process_work(job);
    }
}

async fn non_blocking_worker<P: ExecutorProviderInterface>(
    stealer: P::Stealer,
    state: StateRef<P>,
) {
    state
        .shutdown
        .fold_until_shutdown(stealer, |mut stealer| async {
            let Some(job) = stealer.next().await else { return ControlFlow::Break(()); };

            state.process_work(job);

            ControlFlow::Continue(stealer)
        })
        .await;
}
