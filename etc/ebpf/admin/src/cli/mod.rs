mod bpf;
mod tui;

use bpf::BpfCommand;
use clap::{Args, Parser, Subcommand};
use color_eyre::eyre::Result;
use tui::TuiCommand;

use crate::utils::version;

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
            Command::Bpf(_) => {
                todo!()
            },
        }
    }
}

#[derive(Debug, Parser)]
pub enum Command {
    Tui(TuiCommand),
    #[clap(subcommand)]
    Bpf(BpfCommand),
}
