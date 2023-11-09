use anyhow::{Context, Result};
use clap::Parser;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::ServiceExecutorInterface;
use lightning_node::config::TomlConfigProvider;
use lightning_node::{FinalTypes, WithMockConsensus};
use lightning_rpc::server::Rpc;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_signer::Signer;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::{info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::args::{Args, Command};
use crate::commands::run::CustomStartShutdown;
use crate::commands::{dev, keys, opt, print_config, run};
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
        // In case of spawning the binary with the `SERVICE_ID` env abort the default flow and
        // instead run the code for that service.
        if let Ok(service_id) = std::env::var("SERVICE_ID") {
            tracing_subscriber::fmt::init();
            ServiceExecutor::<FinalTypes>::run_service(
                service_id.parse().expect("SERVICE_ID to be a number"),
            )
            .await;
            std::process::exit(0);
        }

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
        C: Collection<
                ConfigProviderInterface = TomlConfigProvider<C>,
                SignerInterface = Signer<C>,
                RpcInterface = Rpc<C>,
            >,
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
        C: Collection<
                ConfigProviderInterface = TomlConfigProvider<C>,
                SignerInterface = Signer<C>,
                RpcInterface = Rpc<C>,
            >,
    {
        match self.args.cmd {
            Command::Run => run::exec::<C>(config_path, custom_start_shutdown).await,
            Command::Keys(cmd) => keys::exec::<C>(cmd, config_path).await,
            Command::Opt(cmd) => opt::exec::<C>(cmd, config_path).await,
            Command::PrintConfig { default } => print_config::exec::<C>(default, config_path).await,
            Command::Dev(cmd) => dev::exec::<C>(cmd, config_path).await,
        }
    }

    fn setup(&self) {
        // Build the filter from cli args, or override if environment variable is set.
        let mut did_override = false;
        let env_filter = EnvFilter::builder()
            .parse_lossy(match std::env::var("RUST_LOG") {
                // Environment override
                Ok(filter) => {
                    did_override = true;
                    filter
                },
                // Default which is directed by the verbosity flag
                Err(_) => {
                    if self.args.verbose < 3 {
                        // Build the filter, explicitly ignoring noisy dependencies
                        vec![
                            match self.args.verbose {
                                0 => "info",
                                1 => "debug",
                                2 => "trace",
                                _ => unreachable!(),
                            },
                            // Ignore spammy crates
                            "quinn=warn",
                            "anemo=warn",
                            "rustls=warn",
                            "h2=warn",
                        ]
                        .join(",")
                    } else {
                        // Otherwise, trace everything without any explicit ignores
                        "trace".to_string()
                    }
                },
            })
            // Ignore traces from tokio and the runtime for log printing. Namely async task
            // contexts, which are available from tokio console in a human readable way.
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
                .server_addr(([0, 0, 0, 0], 6669))
                .spawn();
            registry.with(console_layer).init();
        } else {
            registry.init();
        }

        if did_override && self.args.verbose != 0 {
            warn!("-v is useless when RUST_LOG override is present");
        }
        if self.args.verbose > 3 {
            warn!("The maximum verbosity level is 3, Parsa.")
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
