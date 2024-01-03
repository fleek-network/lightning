mod args;
pub mod cli;
mod commands;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use lightning_interfaces::ServiceExecutorInterface;
use lightning_node::FinalTypes;
use lightning_service_executor::shim::ServiceExecutor;

use crate::args::Args;

fn main() -> Result<()> {
    let args = Args::parse();
    let cli = Cli::new(args);

    if let Ok(service_id) = std::env::var("SERVICE_ID") {
        // In case of spawning the binary with the `SERVICE_ID` env abort the default flow and
        // instead run the code for that service. We avoid using a runtime so that a service can use
        // its own.
        cli.setup_logging(true);
        ServiceExecutor::<FinalTypes>::run_service(
            service_id.parse().expect("SERVICE_ID to be a number"),
        );
        std::process::exit(0);
    }

    // Create the tokio runtime and execute the cli
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to initialize runtime")
        .block_on(cli.exec())
}
