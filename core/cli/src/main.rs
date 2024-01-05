mod args;
pub mod cli;
mod commands;
mod utils;

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::ServiceExecutorInterface;
use lightning_node::FinalTypes;

use crate::args::Args;

fn main() -> Result<()> {
    let args = Args::parse();
    let cli = Cli::new(args);

    if let Ok(service_id) = std::env::var("SERVICE_ID") {
        // In case of spawning the binary with the `SERVICE_ID` env abort the default flow and
        // instead run the code for that service. We avoid using a runtime so that a service can use
        // its own.
        cli.setup_logging(true);
        <c!(FinalTypes::ServiceExecutorInterface)>::run_service(
            service_id.parse().expect("SERVICE_ID to be a number"),
        );
        std::process::exit(0);
    }

    panic_report::setup! {
        name: "lightning-node".into(),
        version: concat!(
            env!("CARGO_PKG_VERSION"),
            "-",
            compile_time_run::run_command_str!("git", "rev-parse", "HEAD")
        )
        .into(),
        homepage: "https://github.com/fleek-network/lightning",
        contacts: ["Fleek Network Team <reports@fleek.network>".to_string()]
    };

    panic_report::add_context("os", os_info::get().to_string());
    panic_report::add_context(
        "node_start",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );

    // Create the tokio runtime and execute the cli
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to initialize runtime")
        .block_on(cli.exec())
}
