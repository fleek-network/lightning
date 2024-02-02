use serde::{Deserialize, Serialize};
use task::Task;

mod handler;
mod libtorch;
pub mod model;
mod opts;
mod stream;
pub mod task;

pub use opts::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    /// Devices on which tensor computations are run.
    pub device: Device,
    /// Task to execute.
    pub task: Task,
    /// Options for the task.
    pub opts: String,
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    tracing::info!("Initialized ML service!");

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(async move {
            if handler::handle(conn).await.is_err() {
                tracing::info!("there was an error when handling the connection");
            }
        });
    }
}
