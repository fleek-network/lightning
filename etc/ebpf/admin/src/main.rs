#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod action;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod mode;
pub mod tui;
pub mod utils;
pub mod widgets;

use clap::Parser;
use color_eyre::eyre::Result;
use color_eyre::Report;
use ebpf_service::ConfigSource;

use crate::app::App;
use crate::cli::Cli;
use crate::utils::{initialize_logging, initialize_panic_handler, version};

async fn tokio_main() -> Result<()> {
    initialize_logging()?;

    initialize_panic_handler()?;

    let cmd = Cli::parse();
    cmd.exec().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = tokio_main().await {
        eprintln!("{} error: Something went wrong", env!("CARGO_PKG_NAME"));
        return Err(e);
    }

    Ok(())
}
