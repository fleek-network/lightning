use anyhow::{Context, Result};
use clap::Parser;
use lightning_final_bindings::{FinalTypes, UseMockConsensus};
use lightning_interfaces::Collection;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::{info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::args::{Args, Command};
use crate::commands::{admin, dev, keys, opt, print_config, run};
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
        self.setup_logging(false);
        let config_path = self.resolve_config_path()?;
        match self.args.with_mock_consensus {
            true => {
                info!("Using MockConsensus");
                self.run::<UseMockConsensus>(config_path).await
            },
            false => self.run::<FinalTypes>(config_path).await,
        }
    }

    async fn run<C>(self, config_path: ResolvedPathBuf) -> Result<()>
    where
        C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
    {
        match self.args.cmd {
            Command::Run => run::exec::<C>(config_path).await,
            Command::Keys(cmd) => keys::exec::<C>(cmd, config_path).await,
            Command::Opt(cmd) => opt::exec::<C>(cmd, config_path).await,
            Command::PrintConfig { default } => print_config::exec::<C>(default, config_path).await,
            Command::Dev(cmd) => dev::exec::<C>(cmd, config_path).await,
            Command::Admin(cmd) => admin::exec(cmd).await,
        }
    }

    pub fn setup_logging(&self, is_subprocess: bool) {
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
                        [
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

        if self.args.with_console && !is_subprocess {
            // Spawn tokio_console server
            let console_layer = console_subscriber::Builder::default()
                .with_default_env()
                .server_addr(([0, 0, 0, 0], 6669))
                .spawn();
            registry.with(console_layer).init();
        } else {
            registry.init();
        }

        if !is_subprocess {
            if did_override && self.args.verbose != 0 {
                warn!("-v is useless when RUST_LOG override is present");
            }
            if self.args.verbose > 3 {
                warn!("The maximum verbosity level is 3, Parsa.")
            }
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
