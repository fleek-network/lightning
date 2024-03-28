mod cli;
mod commands;

use anyhow::Context;
use clap::Parser;
use cli::Cli;

fn main() -> anyhow::Result<()> {
    let opts = Cli::parse();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build runtime")?
        .block_on(opts.exec())
}
