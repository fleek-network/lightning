mod args;
pub mod cli;
mod commands;
mod shutdown;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

use crate::args::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let cli = Cli::new(args);
    cli.exec().await
}
