use clap::Args;
use color_eyre::eyre::Result;
use ebpf_service::ConfigSource;
use lightning_tui::app::App;

use crate::PATH_CONFIG;

pub async fn exec(cmd: TuiCommand) -> Result<()> {
    let mut app = App::new(
        cmd.tick_rate,
        cmd.frame_rate,
        ConfigSource::new(
            PATH_CONFIG
                .get()
                .cloned()
                .expect("Config to be initialized on start-up"),
        ),
    )?;
    app.run().await
}

#[derive(Args, Debug)]
pub struct TuiCommand {
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
