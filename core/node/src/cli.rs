use std::{marker::PhantomData, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::{arg, Parser, Subcommand};
use draco_interfaces::{ConfigProviderInterface, DracoTypes, Node, WithStartAndShutdown};

use crate::{config::TomlConfigProvider, shutdown::ShutdownController};

#[derive(Parser)]
#[command(about, version)]
pub struct CliArgs {
    /// Path to the toml configuration file
    #[arg(short, long, default_value = "draco.toml")]
    pub config: PathBuf,
    /// Determines that we should be using the mock consensus backend.
    #[arg(long)]
    pub with_mock_consensus: bool,
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

/// Create a new command line application.
pub struct Cli<T: DracoTypes>(CliArgs, PhantomData<T>);

impl<T: DracoTypes> Cli<T> {
    pub fn new(args: CliArgs) -> Self {
        Self(args, PhantomData)
    }
}

impl<T: DracoTypes<ConfigProvider = TomlConfigProvider>> Cli<T>
where
    Node<T>: Send + Sync,
{
    /// Execute the application based on the provided command.
    pub async fn exec(self) -> Result<()> {
        let args = self.0;

        match args.cmd {
            Command::Run {} => Self::run(args.config).await,
            Command::PrintConfig { default } if default => Self::print_default_config().await,
            Command::PrintConfig { .. } => Self::print_config(args.config).await,
        }
    }

    /// Run the node with the provided configuration path.
    async fn run(config_path: PathBuf) -> Result<()> {
        let shutdown_controller = ShutdownController::default();
        shutdown_controller.install_ctrl_c_handler();

        let config = Arc::new(Self::load_or_write_config(config_path).await?);
        let node = Node::<T>::init(config).await?;

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
    async fn print_config(config_path: PathBuf) -> Result<()> {
        let config = Self::load_or_write_config(config_path).await?;
        println!("{}", config.serialize_config());
        Ok(())
    }

    /// Load the configuration file and write the default to the disk.
    async fn load_or_write_config(config_path: PathBuf) -> Result<TomlConfigProvider> {
        let config = TomlConfigProvider::open(&config_path)?;
        Node::<T>::fill_configuration(&config);

        if !config_path.exists() {
            std::fs::write(&config_path, config.serialize_config())?;
        }

        Ok(config)
    }
}
