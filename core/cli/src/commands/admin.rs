use anyhow::{Error, Result};
use clap::{Args, Subcommand};
use ebpf_service::{ConfigSource, PathConfig};
use lightning_tui::app::App;
use once_cell::sync::OnceCell;

pub static PATH_CONFIG: OnceCell<PathConfig> = OnceCell::new();

#[derive(Subcommand)]
pub enum AdminSubCmd {
    /// Start the Admin TUI.
    Tui(TuiCmd),
    /// Start the eBPF Control Application.
    Ebpf,
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
}

pub async fn exec(cmd: AdminSubCmd) -> Result<()> {
    let config = PathConfig::default();
    PATH_CONFIG.set(config).expect("Not to be initialized yet");

    match cmd {
        AdminSubCmd::Tui(cmd) => tui(cmd).await,
        AdminSubCmd::Ebpf => unimplemented!(),
    }
}

pub async fn tui(cmd: TuiCmd) -> Result<()> {
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
