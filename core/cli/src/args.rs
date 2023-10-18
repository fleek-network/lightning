use std::path::PathBuf;

use clap::{arg, ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(about, version)]
pub struct Args {
    /// Path to the toml configuration file
    #[arg(short, long, global = true, default_value_t = String::from("~/.lightning/config.toml") )]
    pub config: String,
    /// Determines that we should be using the mock consensus backend.
    #[arg(long, global = true, default_value_t = false)]
    pub with_mock_consensus: bool,
    /// Enable the Tokio Console asynchronous debugger.
    #[arg(long, global = true, default_value_t = false)]
    pub with_console: bool,
    /// Enable code locations when printing logs.
    #[arg(long, global = true, default_value_t = true)]
    pub with_log_locations: bool,
    /// Increases the level of verbosity (the max level is -vvv).
    #[arg(short, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the full node.
    Run,
    /// Key management utilities.
    #[command(subcommand)]
    Keys(KeySubCmd),
    /// Print the loaded configuration.
    PrintConfig {
        /// Print the default configuration instead of loading the current one.
        #[arg(short, long)]
        default: bool,
    },
    /// Hidden developer subcommands.
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
    /// Print the node's public keys.
    Show,
    /// Generate new private keys.
    /// This command will fail if the keys already exist.
    Generate,
}
