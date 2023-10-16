use anyhow::{Context, Result};
use clap::Parser;
use lightning_interfaces::infu_collection::Collection;
use lightning_node::config::TomlConfigProvider;
use lightning_node::{FinalTypes, WithMockConsensus};
use lightning_signer::Signer;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::info;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::args::{Args, Command};
use crate::commands::run::CustomStartShutdown;
use crate::commands::{dev, keys, print_config, run};
use crate::utils::fs::ensure_parent_exist;

pub struct Cli {
    args: Args,
}

impl Cli {
    pub fn new(args: Args) -> Self {
        Self { args }
    }

    pub fn parse() -> Self {
        Self {
            args: Args::parse(),
        }
    }

    pub async fn exec(self) -> Result<()> {
        self.setup();
        let config_path = self.resolve_config_path()?;
        match self.args.with_mock_consensus {
            true => {
                info!("Using MockConsensus");
                self.run::<WithMockConsensus>(config_path, None).await
            },
            false => self.run::<FinalTypes>(config_path, None).await,
        }
    }

    pub async fn exec_with_custom_start_shutdown<C>(self, cb: CustomStartShutdown<C>) -> Result<()>
    where
        C: Collection<ConfigProviderInterface = TomlConfigProvider<C>, SignerInterface = Signer<C>>,
    {
        self.setup();
        let config_path = self.resolve_config_path()?;
        self.run::<C>(config_path, Some(cb)).await
    }

    async fn run<C>(
        self,
        config_path: ResolvedPathBuf,
        custom_start_shutdown: Option<CustomStartShutdown<C>>,
    ) -> Result<()>
    where
        C: Collection<ConfigProviderInterface = TomlConfigProvider<C>, SignerInterface = Signer<C>>,
    {
        match self.args.cmd {
            Command::Run => run::exec::<C>(config_path, custom_start_shutdown).await,
            Command::Keys(cmd) => keys::exec::<C>(cmd, config_path).await,
            Command::PrintConfig { default } => print_config::exec::<C>(default, config_path).await,
            Command::Dev(cmd) => dev::exec::<C>(cmd, config_path).await,
        }
    }

    fn setup(&self) {
        // Build the filter from cli args, or override if environment variable is set.
        let env_filter = EnvFilter::builder()
            .with_default_directive(
                match self.args.verbose {
                    0 => LevelFilter::INFO,
                    1 => LevelFilter::DEBUG,
                    _2_or_more => LevelFilter::TRACE,
                }
                .into(),
            )
            .from_env_lossy()
            .add_directive("quinn=warn".parse().unwrap())
            .add_directive("anemo=warn".parse().unwrap())
            .add_directive("rustls=warn".parse().unwrap())
            .add_directive("tokio=warn".parse().unwrap())
            .add_directive("runtime=warn".parse().unwrap());

        // Initialize the base logging registry
        let registry = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_file(self.args.with_log_locations)
                .with_filter(env_filter),
        );

        if self.args.with_console {
            // Spawn tokio_console server
            let console_layer = console_subscriber::Builder::default()
                .with_default_env()
                .server_addr(([0, 0, 0, 0], 9001))
                .spawn();
            registry.with(console_layer).init();
        } else {
            registry.init();
        }
    }

    fn resolve_config_path(&self) -> Result<ResolvedPathBuf> {
        let input_path = self.args.config.as_str();
        let config_path = ResolvedPathBuf::try_from(input_path)
            .context(format!("Failed to resolve config path: {input_path}"))?;
        ensure_parent_exist(&config_path)?;
        Ok(config_path)
    }
}
