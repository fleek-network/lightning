mod args;
pub mod cli;
mod commands;
mod shutdown;
mod utils;

use std::process::exit;

use clap::Parser;
use cli::Cli;

use crate::args::Args;

#[tokio::main]
async fn main() {
    let cli = Cli::new(Args::parse());
    if let Err(err) = cli.exec().await {
        eprintln!("Command Failed.");
        eprintln!("Error: {err}");
        exit(-1);
    }
}
