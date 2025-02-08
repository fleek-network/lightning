use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::Parser;
use lightning_cli::args::Args;
use lightning_cli::cli::Cli;
use lightning_interfaces::prelude::*;
use lightning_node_bindings::FullNodeComponents;

fn main() -> Result<()> {
    let args = Args::parse();
    let cli = Cli::new(args);

    if let Ok(service_id) = std::env::var("SERVICE_ID") {
        // In case of spawning the binary with the `SERVICE_ID` env abort the default flow and
        // instead run the code for that service. We avoid using a runtime so that a service can use
        // its own.
        let _log_guard = cli.setup_logging(true);
        <c!(FullNodeComponents::ServiceExecutorInterface)>::run_service(
            service_id.parse().expect("SERVICE_ID to be a number"),
        );
        std::process::exit(0);
    }

    panic_report::setup! {
        name: "lightning-node",
        version: lightning_cli::VERSION.as_str(),
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
