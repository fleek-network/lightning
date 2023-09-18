mod args;
mod cli;
mod commands;
mod config;
mod node;
mod utils;

use std::process::exit;

use clap::Parser;
use cli::Cli;

use crate::args::Args;

#[tokio::main]
async fn main() {
    let cli = Cli::new(Args::parse());
    if let Err(err) = cli.run().await {
        eprintln!("Command Failed.");
        eprintln!("Error: {err}");
        exit(-1);
    }
}
