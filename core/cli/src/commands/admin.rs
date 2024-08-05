use std::fs::File;
use std::path::Path;

use anyhow::{Error, Result};
use clap::{Args, Subcommand};
use lightning_guard::{ConfigSource, PathConfig};
use lightning_tui::app::App;
use lightning_utils::config::LIGHTNING_HOME_DIR;
use once_cell::sync::OnceCell;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::debug;
#[cfg(feature = "dev")]
use tracing::log::LevelFilter;

#[cfg(target_os = "linux")]
use crate::commands::ebpf;
#[cfg(target_os = "linux")]
use crate::commands::ebpf::EbpfCmd;

pub static BIND_PATH: OnceCell<ResolvedPathBuf> = OnceCell::new();
pub static PATH_CONFIG: OnceCell<PathConfig> = OnceCell::new();

#[derive(Subcommand)]
pub enum AdminSubCmd {
    /// Start the Admin TUI.
    Tui(TuiCmd),
    #[cfg(target_os = "linux")]
    #[clap(subcommand)]
    /// Start the eBPF Control Application.
    Ebpf(EbpfCmd),
}

#[derive(Args, Debug)]
pub struct TuiCmd {
    #[arg(
        short,
        long,
        value_name = "FLOAT",
        help = "Tick rate, i.e. number of ticks per second",
        default_value_t = 1.0
    )]
    pub tick_rate: f64,

    #[arg(
        short,
        long,
        value_name = "FLOAT",
        help = "Frame rate, i.e. number of frames per second",
        default_value_t = 4.0
    )]
    pub frame_rate: f64,

    #[arg(
        short,
        long,
        value_name = "INTEGER",
        help = "Size of the logger's buffer. New incoming log records are dropped when the buffer is full",
        default_value_t = 50000
    )]
    pub logger_buffer: usize,
}

pub async fn exec(cmd: AdminSubCmd) -> Result<()> {
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

    // Todo: maybe we could allow users to configure these values via CLI
    // and remove these globals?
    PATH_CONFIG.set(config).expect("Not to be initialized yet");
    BIND_PATH
        .set(
            LIGHTNING_HOME_DIR
                .join("ebpf/ctrl")
                .try_into()
                .expect("Path resolution not to fail"),
        )
        .expect("Not to be initialized yet");

    debug!(
        "binding socket to path {}",
        BIND_PATH.get().expect("To be initialized").display()
    );

    match cmd {
        AdminSubCmd::Tui(cmd) => tui(cmd).await,
        #[cfg(target_os = "linux")]
        AdminSubCmd::Ebpf(cmd) => ebpf::exec(cmd),
    }
}

pub async fn tui(cmd: TuiCmd) -> Result<()> {
    #[cfg(feature = "dev")]
    {
        let _ = tui_logger::init_logger(LevelFilter::Trace);
        tui_logger::set_default_level(LevelFilter::Trace);
        tui_logger::set_hot_buffer_depth(cmd.logger_buffer);
        tui_logger::move_events();
    }

    let mut app = App::new(
        cmd.tick_rate,
        cmd.frame_rate,
        ConfigSource::new(
            PATH_CONFIG
                .get()
                .cloned()
                .expect("Config to be initialized on start-up"),
        ),
    )
    .map_err(|e| Error::msg(e.to_string()))?;
    app.run().await.map_err(|e| Error::msg(e.to_string()))
}
