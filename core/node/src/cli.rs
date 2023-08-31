use std::fs::{self, File};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use clap::{arg, ArgAction, Parser, Subcommand};
use fleek_crypto::{ConsensusSecretKey, NodeSecretKey, PublicKey, SecretKey};
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::{ConfigProviderInterface, SignerInterface};
use lightning_signer::Signer;
use log::LevelFilter;
use resolved_pathbuf::ResolvedPathBuf;
use simplelog::{
    ColorChoice,
    CombinedLogger,
    ConfigBuilder,
    TermLogger,
    TerminalMode,
    WriteLogger,
};

use crate::config::TomlConfigProvider;
use crate::shutdown::ShutdownController;

const DEFAULT_CONFIG_PATH: &str = "~/.lightning/config.toml";

#[derive(Parser)]
#[command(about, version)]
pub struct CliArgs {
    /// Path to the toml configuration file
    #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
    pub config: PathBuf,
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
    /// Handle keys.
    #[command(subcommand)]
    Keys(Keys),
    ///
    /// Print the loaded configuration.
    ///
    /// By default this command prints the loaded configuration.
    PrintConfig {
        /// Print the default configuration and not the loaded configuration.
        #[arg(long)]
        default: bool,
    },
    /// Initialize the node without starting it.
    DevInitOnly,
    /// Dump the infusion graph of the node instance.
    DevDumpGraph {
        /// Only show the initialization order.
        #[arg(long)]
        show_order: bool,
    },
}

#[derive(Subcommand)]
pub enum Keys {
    /// Print the keys.
    Show,
    /// Generate new keys.
    /// This command will fail if the keys already exist.
    Generate,
}

/// Create a new command line application.
pub struct Cli<C: Collection>(CliArgs, PhantomData<C>);

impl<C: Collection> Cli<C> {
    pub fn new(args: CliArgs) -> Self {
        Self(args, PhantomData)
    }

    fn setup(&self) {
        let args = &self.0;

        let log_level = args.verbose;
        let log_filter = match log_level {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _3_or_more => LevelFilter::Trace,
        };

        // Add ignore for process subdag because Narwhal prints it as an err everytime it
        // successfully processes a new sub_dag
        let logger_config = ConfigBuilder::new()
            .add_filter_ignore_str("narwhal_consensus::bullshark")
            .add_filter_ignore_str("anemo")
            .set_target_level(LevelFilter::Error)
            .set_location_level(if args.log_location {
                LevelFilter::Error
            } else {
                LevelFilter::Trace
            })
            .build();

        let date = Local::now();
        let log_file = std::env::temp_dir().join(format!(
            "lightning-{}.log",
            date.format("%Y-%m-%d-%H:%M:%S")
        ));

        CombinedLogger::init(vec![
            TermLogger::new(
                log_filter,
                logger_config,
                TerminalMode::Mixed,
                ColorChoice::Auto,
            ),
            WriteLogger::new(
                LevelFilter::Trace,
                simplelog::Config::default(),
                File::create(log_file).unwrap(),
            ),
        ])
        .unwrap();
    }
}

impl<C: Collection<ConfigProviderInterface = TomlConfigProvider<C>, SignerInterface = Signer<C>>>
    Cli<C>
{
    pub async fn parse_and_exec() -> Result<()> {
        let args = CliArgs::parse();
        Self::new(args).exec().await
    }

    /// Execute the application based on the provided command.
    pub async fn exec(self) -> Result<()> {
        self.setup();

        let args = self.0;

        let config_path =
            ResolvedPathBuf::try_from(args.config.clone()).expect("Failed to resolve config path");

        if let Some(parent_dir) = config_path.parent() {
            let _ = fs::create_dir_all(parent_dir);
        }

        match args.cmd {
            Command::Run {} => Self::run(config_path).await,
            Command::Keys(Keys::Generate) => Self::generate_keys(config_path).await,
            Command::Keys(Keys::Show) => Self::show_keys(config_path).await,
            Command::PrintConfig { default } if default => Self::print_default_config().await,
            Command::PrintConfig { .. } => Self::print_config(config_path).await,
            Command::DevInitOnly => {
                let config = Self::load_or_write_config(config_path).await?;
                Node::<C>::init(config)
                    .map_err(|e| anyhow::anyhow!("Could not start the node: {e}"))?;
                Ok(())
            },
            Command::DevDumpGraph { show_order } if show_order => {
                let graph = C::build_graph();
                let sorted = graph.sort()?;
                for (i, tag) in sorted.iter().enumerate() {
                    println!(
                        "{:0width$}  {tag}\n      = {ty}",
                        i + 1,
                        width = 2,
                        tag = tag.trait_name(),
                        ty = tag.type_name()
                    );
                }
                Ok(())
            },
            Command::DevDumpGraph { .. } => {
                let graph = C::build_graph();
                let mermaid = graph.mermaid("Lightning Dependency Graph");
                println!("{mermaid}");
                Ok(())
            },
        }
    }

    /// Run the node with the provided configuration path.
    async fn run(config_path: ResolvedPathBuf) -> Result<()> {
        let shutdown_controller = ShutdownController::default();
        shutdown_controller.install_ctrl_c_handler();

        let config = Self::load_or_write_config(config_path).await?;
        let node = Node::<C>::init(config)
            .map_err(|e| anyhow::anyhow!("Could not start the node: {e}"))?;

        node.start().await;

        shutdown_controller.wait_for_shutdown().await;
        node.shutdown().await;

        Ok(())
    }

    /// Print the default configuration for the node, this function does not
    /// create a new file.
    async fn print_default_config() -> Result<()> {
        let config = TomlConfigProvider::<C>::default();
        Node::<C>::fill_configuration(&config);
        println!("{}", config.serialize_config());
        Ok(())
    }

    /// Print the configuration from the given path.
    async fn print_config(config_path: ResolvedPathBuf) -> Result<()> {
        let config = Self::load_or_write_config(config_path).await?;
        println!("{}", config.serialize_config());
        Ok(())
    }

    /// Load the configuration file and write the default to the disk.
    async fn load_or_write_config(config_path: ResolvedPathBuf) -> Result<TomlConfigProvider<C>> {
        let config = TomlConfigProvider::open(&config_path)?;
        Node::<C>::fill_configuration(&config);

        if !config_path.exists() {
            std::fs::write(&config_path, config.serialize_config())?;
        }

        Ok(config)
    }

    async fn generate_keys(config_path: ResolvedPathBuf) -> Result<()> {
        let config = Arc::new(Self::load_or_write_config(config_path).await?);
        let signer_config = config.get::<C::SignerInterface>();

        if signer_config.node_key_path.exists() {
            return Err(anyhow!(
                "Node secret key exists at specified path. Not generating keys."
            ));
        }

        if signer_config.consensus_key_path.exists() {
            return Err(anyhow!(
                "Consensus secret key exists at specified path. Not generating keys."
            ));
        }

        match Signer::<C>::generate_node_key(&signer_config.node_key_path) {
            Ok(_) => println!(
                "Successfully created node secret key at: {:?}",
                signer_config.node_key_path
            ),
            Err(err) => return Err(anyhow!("Failed to create node secret key: {err:?}")),
        };

        match Signer::<C>::generate_consensus_key(&signer_config.consensus_key_path) {
            Ok(_) => println!(
                "Successfully created consensus secret key at: {:?}",
                signer_config.consensus_key_path
            ),
            Err(err) => {
                fs::remove_file(signer_config.node_key_path)?;
                return Err(anyhow!("Failed to create consensus secret key: {err:?}"));
            },
        };
        Ok(())
    }

    async fn show_keys(config_path: ResolvedPathBuf) -> Result<()> {
        let config = Arc::new(Self::load_or_write_config(config_path).await?);
        let signer_config = config.get::<C::SignerInterface>();
        if signer_config.node_key_path.exists() {
            let node_secret_key = fs::read_to_string(&signer_config.node_key_path)
                .with_context(|| "Failed to read node pem file")?;
            let node_secret_key = NodeSecretKey::decode_pem(&node_secret_key)
                .with_context(|| "Failed to decode node pem file")?;
            println!("Node Public Key: {}", node_secret_key.to_pk().to_base64());
        } else {
            eprintln!("Node Public Key: does not exist");
        }

        if signer_config.consensus_key_path.exists() {
            let consensus_secret_key = fs::read_to_string(&signer_config.consensus_key_path)
                .with_context(|| "Failed to read consensus pem file")?;
            let consensus_secret_key = ConsensusSecretKey::decode_pem(&consensus_secret_key)
                .with_context(|| "Failed to decode consensus pem file")?;
            println!(
                "Consensus Public Key: {}",
                consensus_secret_key.to_pk().to_base64()
            );
        } else {
            eprintln!("Consensus Public Key: does not exist");
        }
        Ok(())
    }
}

#[test]
fn some() {
    let config = PathBuf::from("/config.toml");

    config.parent().unwrap();
}
