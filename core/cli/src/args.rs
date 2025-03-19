use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{arg, ArgAction, Parser, Subcommand, ValueEnum};
use lightning_application::network::Network;
use lightning_utils::config::LIGHTNING_HOME_DIR;

use crate::commands::admin::AdminSubCmd;

#[derive(Parser)]
#[command(about, name = "lightning-node", version = crate::VERSION.clone())]
pub struct Args {
    /// Path to the toml configuration file
    #[arg(short, long, global = true, default_value_t = String::from(LIGHTNING_HOME_DIR.join("config.toml").to_string_lossy().as_ref()) )]
    pub config: String,
    /// Determines that we should be using the mock consensus backend.
    #[arg(long, global = true, default_value_t = false)]
    pub with_mock_consensus: bool,
    /// Enable the Tokio Console asynchronous debugger.
    #[arg(long, global = true, default_value_t = false)]
    pub with_console: bool,
    /// Enable code locations when printing logs.
    #[arg(long, global = true, default_value_t = false)]
    pub with_log_locations: bool,
    /// Increases the level of verbosity (the max level is -vvv).
    #[arg(short, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum NetworkArg {
    LocalnetExample,
    TestnetStable,
    TestnetStaging,
}

impl From<NetworkArg> for Network {
    fn from(network: NetworkArg) -> Self {
        match network {
            NetworkArg::LocalnetExample => Network::LocalnetExample,
            NetworkArg::TestnetStable => Network::TestnetStable,
            NetworkArg::TestnetStaging => Network::TestnetStable,
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the full node.
    Run,
    /// Initialize the node configuration and genesis block.
    Init {
        /// The built-in network genesis configuration to use. If dev is set, this is not
        /// required and will default to the local dev genesis configuration.
        #[clap(value_enum, long, short)]
        network: Option<NetworkArg>,
        /// Whether to start the node with dev configuration. If set, the network will default to
        /// the local dev genesis configuration, and the dev configuration will be applied.
        #[clap(short, long)]
        dev: bool,
        /// Whether to force overwriting the config files.
        #[clap(short, long)]
        force: bool,
        /// Whether to not generate keys during initialization. The default is to generate keys.
        #[clap(long)]
        no_generate_keys: bool,
        /// Set RPC listen address in the generated configuration file.
        #[clap(long)]
        rpc_address: Option<SocketAddr>,
        /// Set handshake HTTP listen address in the generated configuration file.
        #[clap(long)]
        handshake_http_address: Option<SocketAddr>,
    },
    /// Key management utilities.
    #[command(subcommand)]
    Keys(KeySubCmd),
    /// Opt into or opt out of network participation.
    #[command(subcommand)]
    Opt(OptSubCmd),
    /// Print the loaded configuration.
    PrintConfig {
        /// Print the default configuration instead of loading the current one.
        #[arg(short, long)]
        default: bool,
    },
    /// Hidden developer subcommands.
    #[command(subcommand, hide = true)]
    Dev(DevSubCmd),
    /// Applications for administrators.
    #[command(subcommand)]
    Admin(AdminSubCmd),
    /// Generate shell completions
    Completions { shell: clap_complete::shells::Shell },
}

#[derive(Subcommand, Clone)]
pub enum DevSubCmd {
    /// Dump the mermaid dependency graph of services.
    DepGraph,
    /// Store the provided files to the blockstore.
    Store { input: Vec<PathBuf> },
    /// Fetch a content from the blockstore server of the remote.
    Fetch {
        /// The node index of the node.
        #[arg(short, long)]
        remote: u32,
        /// The Blake3 hash of the content that we want to download.
        hash: String,
    },
    /// Clear the state tree and rebuild it from scratch.
    ResetStateTree,
}

#[derive(Subcommand, PartialEq, Eq)]
pub enum KeySubCmd {
    /// Print the node's public keys.
    Show,
    /// Generate new private keys.
    /// This command will fail if the keys already exist.
    Generate,
}

#[derive(Subcommand)]
pub enum OptSubCmd {
    /// Opt into network participation.
    In,
    /// Opt out of network participation. Run this command before shutting down your node.
    Out,
    /// Query the participation status of your node.
    Status,
}
