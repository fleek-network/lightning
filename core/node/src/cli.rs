use std::{fs, marker::PhantomData, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context, Result};
use clap::{arg, ArgAction, Parser, Subcommand};
use fleek_crypto::{ConsensusSecretKey, NodeSecretKey, PublicKey, SecretKey};
use lightning_interfaces::{
    ConfigProviderInterface, LightningTypes, Node, SignerInterface, WithStartAndShutdown,
};
use lightning_signer::Signer;
use resolved_pathbuf::ResolvedPathBuf;

use crate::{config::TomlConfigProvider, shutdown::ShutdownController};

#[derive(Parser)]
#[command(about, version)]
pub struct CliArgs {
    /// Path to the toml configuration file
    #[arg(short, long, default_value = "lightning.toml")]
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
pub struct Cli<T: LightningTypes>(CliArgs, PhantomData<T>);

impl<T: LightningTypes> Cli<T> {
    pub fn new(args: CliArgs) -> Self {
        Self(args, PhantomData)
    }
}

impl<T: LightningTypes<ConfigProvider = TomlConfigProvider>> Cli<T>
where
    Node<T>: Send + Sync,
{
    /// Execute the application based on the provided command.
    pub async fn exec(self) -> Result<()> {
        let args = self.0;

        let config_path =
            ResolvedPathBuf::try_from(args.config.clone()).expect("Failed to resolve config path");

        match args.cmd {
            Command::Run {} => Self::run(config_path).await,
            Command::Keys(Keys::Generate) => Self::generate_keys(config_path).await,
            Command::Keys(Keys::Show) => Self::show_keys(config_path).await,
            Command::PrintConfig { default } if default => Self::print_default_config().await,
            Command::PrintConfig { .. } => Self::print_config(config_path).await,
        }
    }

    /// Run the node with the provided configuration path.
    async fn run(config_path: ResolvedPathBuf) -> Result<()> {
        let shutdown_controller = ShutdownController::default();
        shutdown_controller.install_ctrl_c_handler();

        let config = Arc::new(Self::load_or_write_config(config_path).await?);
        let node = Node::<T>::init(config)?;

        node.start().await;

        shutdown_controller.wait_for_shutdown().await;
        node.shutdown().await;

        Ok(())
    }

    /// Print the default configuration for the node, this function does not
    /// create a new file.
    async fn print_default_config() -> Result<()> {
        let config = TomlConfigProvider::default();
        Node::<T>::fill_configuration(&config);
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
    async fn load_or_write_config(config_path: ResolvedPathBuf) -> Result<TomlConfigProvider> {
        let config = TomlConfigProvider::open(&config_path)?;
        Node::<T>::fill_configuration(&config);

        if !config_path.exists() {
            std::fs::write(&config_path, config.serialize_config())?;
        }

        Ok(config)
    }

    async fn generate_keys(config_path: ResolvedPathBuf) -> Result<()> {
        let config = Arc::new(Self::load_or_write_config(config_path).await?);
        let signer_config = config.get::<Signer>();
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
        match Signer::generate_node_key(&signer_config.node_key_path) {
            Ok(_) => println!(
                "Successfully created node secret key at: {:?}",
                signer_config.node_key_path
            ),
            Err(err) => eprintln!("Failed to create node secret key: {err:?}"),
        };
        match Signer::generate_consensus_key(&signer_config.consensus_key_path) {
            Ok(_) => println!(
                "Successfully created consensus secret key at: {:?}",
                signer_config.consensus_key_path
            ),
            Err(err) => {
                fs::remove_file(signer_config.node_key_path)?;
                eprintln!("Failed to create consensus secret key: {err:?}");
            },
        };
        Ok(())
    }

    async fn show_keys(config_path: ResolvedPathBuf) -> Result<()> {
        let config = Arc::new(Self::load_or_write_config(config_path).await?);
        let signer_config = config.get::<Signer>();
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
