mod cli;
mod commands;

use anyhow::Context;
use clap::Parser;
use cli::Cli;
use color_eyre::{Report, Result};
use ebpf_service::ConfigSource;

fn main() -> Result<()> {
    lightning_tui::utils::initialize_logging()?;

    lightning_tui::utils::initialize_panic_handler()?;

    ConfigSource::create_config().unwrap();

    let cmd = Cli::parse();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build runtime")
        .map_err(Report::msg)?
        .block_on(cmd.exec())
}
