mod cli;
mod commands;

use std::fs::File;
use std::path::Path;

use anyhow::Context;
use clap::Parser;
use cli::Cli;
use color_eyre::{Report, Result};
use ebpf_service::PathConfig;
use once_cell::sync::OnceCell;
use resolved_pathbuf::ResolvedPathBuf;

pub static PATH_CONFIG: OnceCell<PathConfig> = OnceCell::new();
pub static BIND_PATH: OnceCell<ResolvedPathBuf> = OnceCell::new();

fn main() -> Result<()> {
    lightning_tui::utils::initialize_logging()?;

    lightning_tui::utils::initialize_panic_handler()?;

    let cmd = Cli::parse();

    let config = PathConfig::default();

    if !Path::new(config.tmp_dir.as_path()).try_exists()? {
        std::fs::create_dir_all(config.tmp_dir.as_path())?;
    }

    if !Path::new(config.profiles_dir.as_path()).try_exists()? {
        std::fs::create_dir_all(config.profiles_dir.as_path())?;
    }

    if !Path::new(config.packet_filter.as_path()).try_exists()? {
        File::create(config.packet_filter.as_path())?;
    }

    PATH_CONFIG.set(config).expect("Not to be initialized yet");
    BIND_PATH
        .set(
            ResolvedPathBuf::try_from("~/.lightning/ebpf/ctrl")
                .expect("Path resolution not to fail"),
        )
        .expect("Not to be initialized yet");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build runtime")
        .map_err(Report::msg)?
        .block_on(cmd.exec())
}
