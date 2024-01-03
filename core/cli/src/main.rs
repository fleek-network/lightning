mod args;
pub mod cli;
mod commands;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use compile_time_run::run_command_str;
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

        // Ignore SIGINT signals by default to avoid conflicting on ctrlc shutdown
        unsafe {
            use nix::sys::signal::{signal, SigHandler, Signal};
            signal(Signal::SIGINT, SigHandler::SigIgn).expect("failed to ignore SIGINT")
        };

        // Start the service
        ServiceExecutor::<FinalTypes>::run_service(
            service_id.parse().expect("SERVICE_ID to be a number"),
        );
        std::process::exit(0);
    }

    human_panic::setup_panic!(Metadata {
        version: concat!(
            env!("CARGO_PKG_VERSION"),
            "-",
            run_command_str!("git", "rev-parse", "HEAD")
        )
        .into(),
        name: "lightning-node".into(),
        authors: "Fleek Network Team <reports@fleek.network>".into(),
        homepage: "https://github.com/fleek-network/lightning".into()
    });

    // Create the tokio runtime and execute the cli
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to initialize runtime")
        .block_on(cli.exec())
}
