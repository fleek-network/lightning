use anyhow::Result;
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

    pub async fn run(self) -> Result<()> {
        self.setup();

        let config_path = ResolvedPathBuf::try_from(self.args.config.as_str())
            .expect("Failed to resolve config path");
        ensure_parent_exist(&config_path)?;

        match self.args.cmd {
            Command::Run => run::exec().await,
            Command::Key(cmd) => key::exec(cmd).await,
            Command::PrintConfig { default } => print_config::exec(default, config_path).await,
            Command::Dev(cmd) => dev::exec(cmd).await,
        }
    }

    fn setup(&self) {
        let log_filter = match self.args.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _3_or_more => LevelFilter::Trace,
        };

        let stdout = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{d(%Y-%m-%d %H:%M:%S)(utc)} | {h({l}):5.5} | {M} - {f}:{L} - {m}{n}",
            )))
            .build();

        let log_file = std::env::temp_dir().join("lightning.log");
        let size_trigger = SizeTrigger::new(150 * 1024 * 1024); // 150 MB
        let roller = FixedWindowRoller::builder()
            .build(
                std::env::temp_dir()
                    .join("lightning.log.{}.gz")
                    .to_str()
                    .unwrap(),
                10,
            )
            .unwrap();
        let policy = CompoundPolicy::new(Box::new(size_trigger), Box::new(roller));

        let rolling_appender = RollingFileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{d(%Y-%m-%d %H:%M:%S)(utc)} | {h({l}):5.5} | {M} - {f}:{L} - {m}{n}",
            )))
            .build(log_file, Box::new(policy))
            .unwrap();

        let mut custom_loggers = Vec::new();
        // the bullshark logger is ignored for console only
        let bullshark_ignore_logger = Logger::builder()
            .appenders(vec!["file"])
            .additive(false)
            .build("narwhal_consensus::bullshark", LevelFilter::Trace);

        custom_loggers.push(bullshark_ignore_logger);

        let custom_filter = CustomLogFilter::new()
            .insert("quinn", LevelFilter::Off)
            .insert("anemo", LevelFilter::Off);

        let config = Config::builder()
            .appender(
                Appender::builder()
                    .filter(Box::new(custom_filter.clone()))
                    .build("file", Box::new(rolling_appender)),
            )
            .appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(log_filter)))
                    .filter(Box::new(custom_filter))
                    .build("stdout", Box::new(stdout)),
            )
            .loggers(custom_loggers)
            .build(
                Root::builder()
                    .appender("file")
                    .appender("stdout")
                    .build(LevelFilter::Trace),
            )
            .unwrap();

        log4rs::init_config(config).unwrap();
    }
}
