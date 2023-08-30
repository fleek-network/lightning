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
    while let Some(work) = stealer.next_blocking() {
        state.process_work(work);
    }
}

async fn non_blocking_worker<P: ExecutorProviderInterface>(
    mut stealer: P::Stealer,
    state: StateRef<P>,
) {
    while let Some(work) = stealer.next().await {
        state.process_work(work);
    }
}
