use std::path::PathBuf;

use clap::{arg, Parser, Subcommand};

#[derive(Parser)]
#[command(about, version)]
pub struct CliArgs {
    /// Path to the toml configuration file
    #[arg(short, long, default_value = "draco.toml")]
    pub config: PathBuf,
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the node.
    Run,
    /// Print the loaded configuration.
    ///
    /// By default this command prints the loaded configuration.
    PrintConfig {
        /// Print the default configuration and not the loaded configuration.
        #[arg(long)]
        default: bool,
    },
}
