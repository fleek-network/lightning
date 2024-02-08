use serde::{Deserialize, Serialize};

mod handler;
mod libtorch;
pub mod model;
mod opts;
mod stream;
pub mod task;

pub use opts::*;

use crate::task::{Run, Train};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    /// Task to execute.
    Run(Run),
    Train(Train),
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    tracing::info!("Initialized AI service!");

    let mut listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handler::handle(conn).await {
                tracing::info!("there was an error when handling the connection: {e:?}");
            }
        });
    }
}
