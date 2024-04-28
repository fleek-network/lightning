use clap::Parser;
use color_eyre::eyre::Result;
use lightning_tui::utils::version;

use crate::commands::ebpf::{self, EbpfCommand};
use crate::commands::tui::{self, TuiCommand};

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
    #[clap(subcommand)]
    pub(crate) command: Command,
}

impl Cli {
    pub async fn exec(self) -> Result<()> {
        match self.command {
            Command::Tui(cmd) => tui::exec(cmd).await,
            Command::Ebpf(cmd) => ebpf::exec(cmd),
        }
    }
}

#[derive(Debug, Parser)]
pub enum Command {
    Tui(TuiCommand),
    #[clap(subcommand)]
    Ebpf(EbpfCommand),
}
