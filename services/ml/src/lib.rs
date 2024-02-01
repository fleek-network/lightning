use opts::Device;
use serde::{Deserialize, Serialize};

mod model;
mod opts;
mod run;
mod stream;
mod train;

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    device: Device,
    task: Task,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Task {
    Train {
        model: model::Opts,
        session: train::Opts,
    },
    Run {
        model: model::Opts,
        session: run::Opts,
    },
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    tracing::info!("Initialized IPFS fetcher service!");

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(_conn) = listener.accept().await {}
}
