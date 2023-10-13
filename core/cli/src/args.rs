use std::path::PathBuf;

use clap::{arg, ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(about, version)]
pub struct Args {
    /// Path to the toml configuration file
    #[arg(short, long, default_value_t = format!("{}/.lightning/config.toml", env!("HOME")))]
    pub config: String,
    /// Determines that we should be using the mock consensus backend.
    #[arg(long)]
    pub with_mock_consensus: bool,
    /// Increases the level of verbosity (the max level is -vvv).
    #[clap(short, action = ArgAction::Count)]
    pub verbose: u8,
    /// Print code location on console logs
    #[arg(long)]
    pub log_location: bool,
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the node.
    Run,
    /// Key management utilities.
    #[command(subcommand)]
    Keys(KeySubCmd),
    /// Print the loaded configuration.
    ///
    /// By default this command prints the loaded configuration.
    PrintConfig {
        /// Print the default configuration and not the loaded configuration.
        #[arg(long)]
        default: bool,
    },
    /// Dev only sub commands. These are hidden by default.
    #[command(subcommand, hide = true)]
    Dev(DevSubCmd),
}

#[derive(Subcommand, Clone)]
pub enum DevSubCmd {
    /// Initialize every service without starting the node.
    InitOnly,
    /// Show the order at which the execution will happen.
    ShowOrder,
    /// Dump the mermaid dependency graph of services.
    DepGraph,
    /// Store the provided files to the blockstore.
    Store { input: Vec<PathBuf> },
}

#[derive(Subcommand)]
pub enum KeySubCmd {
    /// Print the keys.
    Show,
    /// Generate new keys.
    /// This command will fail if the keys already exist.
    Generate,
}
