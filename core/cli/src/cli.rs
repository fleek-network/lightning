use std::path::PathBuf;

use anyhow::Result;

use crate::args::{Args, Command};
use crate::commands::{dev, key, print_config, run};
use crate::utils::ensure_parent_exist;

pub struct Cli {
    args: Args,
}

impl Cli {
    pub fn new(args: Args) -> Self {
        Self { args }
    }

    pub async fn run(self) -> Result<()> {
        let config_path = PathBuf::from(&self.args.config);
        ensure_parent_exist(&config_path)?;

        match self.args.cmd {
            Command::Run => run::exec().await,
            Command::Key(cmd) => key::exec(cmd).await,
            Command::PrintConfig { default } => print_config::exec(default, config_path).await,
            Command::Dev(cmd) => dev::exec(cmd).await,
        }
    }
}
