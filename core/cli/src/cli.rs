use anyhow::{Context, Result};
use clap::Parser;
use lightning_interfaces::infu_collection::Collection;
use lightning_node::config::TomlConfigProvider;
use lightning_node::{FinalTypes, WithMockConsensus};
use lightning_signer::Signer;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::runtime::Logger;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::filter::threshold::ThresholdFilter;
use resolved_pathbuf::ResolvedPathBuf;

use crate::args::{Args, Command};
use crate::commands::run::CustomStartShutdown;
use crate::commands::{dev, key, print_config, run};
use crate::utils::fs::ensure_parent_exist;
use crate::utils::log_filter::CustomLogFilter;

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
                log::info!("Using MockConsensus");
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
            Command::Key(cmd) => key::exec::<C>(cmd, config_path).await,
            Command::PrintConfig { default } => print_config::exec::<C>(default, config_path).await,
            Command::Dev(cmd) => dev::exec::<C>(cmd, config_path).await,
        }
    }

    fn setup(&self) {
        let log_filter = match self.args.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _3_or_more => LevelFilter::Trace,
        };

        let fmt = "{d(%Y-%m-%d %H:%M:%S)(utc)} | {h({l}):5.5} | {M} - {f}:{L} - {m}{n}";

        let stdout_appender = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(fmt)))
            .build();

        let file_rolling_appender = RollingFileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(fmt)))
            .build(
                std::env::temp_dir().join("lightning.log"), // /tmp/lightning.log
                // compound policy
                Box::new(CompoundPolicy::new(
                    // trigger
                    Box::new(SizeTrigger::new(150 * 1024 * 1024)), // 150 MB
                    // roller
                    Box::new(
                        FixedWindowRoller::builder()
                            .build(
                                std::env::temp_dir()
                                    .join("lightning.log.{}.gz")
                                    .to_str()
                                    .unwrap(),
                                10,
                            )
                            .unwrap(),
                    ),
                )),
            )
            .unwrap();

        // the bullshark logger is ignored for console only
        let bullshark_ignore_logger = Logger::builder()
            .appenders(vec!["file"])
            .additive(false)
            .build("narwhal_consensus::bullshark", LevelFilter::Trace);

        let custom_filter = CustomLogFilter::new()
            .insert("quinn", LevelFilter::Off)
            .insert("anemo", LevelFilter::Off);

        let config = Config::builder()
            .appender(
                Appender::builder()
                    .filter(Box::new(custom_filter.clone()))
                    .build("file", Box::new(file_rolling_appender)),
            )
            .appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(log_filter)))
                    .filter(Box::new(custom_filter))
                    .build("stdout", Box::new(stdout_appender)),
            )
            .loggers(vec![bullshark_ignore_logger])
            .build(
                Root::builder()
                    .appender("file")
                    .appender("stdout")
                    .build(LevelFilter::Trace),
            )
            .unwrap();

        log4rs::init_config(config).unwrap();
    }

    fn resolve_config_path(&self) -> Result<ResolvedPathBuf> {
        let input_path = self.args.config.as_str();
        let config_path = ResolvedPathBuf::try_from(input_path)
            .context(format!("Failed to resolve config path: {input_path}"))?;
        ensure_parent_exist(&config_path)?;
        Ok(config_path)
    }
}
