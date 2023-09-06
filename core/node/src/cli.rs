// TODO(qti3e): Split the CLI and make it more "beautiful".

use std::fs;
use std::future::Future;
use std::io::Read;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::{arg, ArgAction, Parser, Subcommand};
use fleek_crypto::{ConsensusSecretKey, NodeSecretKey, PublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore_server::BlockStoreServer;
use lightning_consensus::notify_container::get_value;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigProviderInterface,
    IncrementalPutInterface,
    SignerInterface,
};
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

use crate::config::TomlConfigProvider;
use crate::shutdown::ShutdownController;
use crate::testnet_sync;

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
    /// Print the loaded configuration.
    ///
    /// By default this command prints the loaded configuration.
    PrintConfig {
        /// Print the default configuration and not the loaded configuration.
        #[arg(long)]
        default: bool,
    },
    /// Dev only sub commands. These are hidden by default.
    #[command(subcommand, hide = true)]
    Dev(Dev),
}

#[derive(Subcommand, Clone)]
pub enum Dev {
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
pub enum Keys {
    /// Print the keys.
    Show,
    /// Generate new keys.
    /// This command will fail if the keys already exist.
    Generate,
}

/// Create a new command line application.
pub struct Cli<C: Collection>(CliArgs, Option<CustomStartShutdown<C>>);
pub type CustomStartShutdown<C> = Box<dyn for<'a> Fn(&'a Node<C>, bool) -> Fut<'a>>;
pub type Fut<'a> = Pin<Box<dyn Future<Output = ()> + 'a>>;

impl<C: Collection> Cli<C> {
    pub fn new(args: CliArgs) -> Self {
        Self(args, None)
    }

    pub fn parse() -> Self {
        Self::new(CliArgs::parse())
    }

    pub fn with_custom_start_shutdown(self, cb: CustomStartShutdown<C>) -> Self {
        Self(self.0, Some(cb))
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

        let stdout = ConsoleAppender::builder().build();

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
                "{d(%Y-%m-%d %H:%M:%S)(utc)} [{l}] {M}:{m}{n}",
            )))
            .build(log_file, Box::new(policy))
            .unwrap();

        let ignore_logger_names = vec!["anemo", "anemo_tower", "quinn", "quinn_proto"];
        let mut ignore_loggers: Vec<Logger> = ignore_logger_names
            .iter()
            .map(|&name| Logger::builder().build(name, LevelFilter::Off))
            .collect();

        // the bullshark logger is ignored for console only
        let bullshark_ignore_logger = Logger::builder()
            .appenders(vec!["file"])
            .additive(false)
            .build("narwhal_consensus::bullshark", LevelFilter::Trace);

        ignore_loggers.push(bullshark_ignore_logger);

        let config = Config::builder()
            .appender(Appender::builder().build("file", Box::new(rolling_appender)))
            .appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(log_filter)))
                    .build("stdout", Box::new(stdout)),
            )
            .loggers(ignore_loggers)
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

impl<C: Collection<ConfigProviderInterface = TomlConfigProvider<C>, SignerInterface = Signer<C>>>
    Cli<C>
{
    /// Execute the application based on the provided command.
    pub async fn exec(self) -> Result<()> {
        self.setup();

        let args = &self.0;

        let config_path =
            ResolvedPathBuf::try_from(args.config.clone()).expect("Failed to resolve config path");

        if let Some(parent_dir) = config_path.parent() {
            let _ = fs::create_dir_all(parent_dir);
        }

        match &args.cmd {
            Command::Run {} => self.run(config_path).await,
            Command::Keys(Keys::Generate) => Self::generate_keys(config_path).await,
            Command::Keys(Keys::Show) => Self::show_keys(config_path).await,
            Command::PrintConfig { default } if *default => Self::print_default_config().await,
            Command::PrintConfig { .. } => Self::print_config(config_path).await,
            Command::Dev(cmd) => cmd.exec::<C>(config_path).await,
        }
    }

    /// Run the node with the provided configuration path.
    async fn run(self, config_path: ResolvedPathBuf) -> Result<()> {
        loop {
            let config_path = config_path.clone();
            // testnet sync
            let config = Self::load_or_write_config(config_path.try_into().unwrap()).await?;
            let shutdown_controller = ShutdownController::default();
            shutdown_controller.install_handlers();

            let signer_config = config.get::<Signer<C>>();
            let app_config = config.get::<Application<C>>();
            let blockstore_config = config.get::<Blockstore<C>>();
            let block_server_config = config.get::<BlockStoreServer<C>>();
            if app_config.testnet {
                testnet_sync::sync(
                    signer_config,
                    app_config,
                    blockstore_config,
                    block_server_config,
                )
                .await;
            }

            let node = Node::<C>::init(config)
                .map_err(|e| anyhow::anyhow!("Could not start the node: {e}"))?;

            let reset_notifier = get_value();

            node.start().await;

            tokio::select! {
                _ = shutdown_controller.wait_for_shutdown() =>{
                    node.shutdown().await;
                    break;
                },
                _ = reset_notifier.notified() => {
                    node.shutdown().await;
                    continue;
                }

            }
        }

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

impl Dev {
    pub async fn exec<
        C: Collection<ConfigProviderInterface = TomlConfigProvider<C>, SignerInterface = Signer<C>>,
    >(
        &self,
        config_path: ResolvedPathBuf,
    ) -> Result<()> {
        match self {
            Dev::InitOnly => {
                let config = Cli::<C>::load_or_write_config(config_path).await?;
                Node::<C>::init(config)
                    .map_err(|e| anyhow::anyhow!("Could not start the node: {e}"))?;
                Ok(())
            },
            Dev::ShowOrder => {
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
            Dev::DepGraph => {
                let graph = C::build_graph();
                let mermaid = graph.mermaid("Lightning Dependency Graph");
                println!("{mermaid}");
                Ok(())
            },
            Dev::Store { input } => {
                let config = Cli::<C>::load_or_write_config(config_path).await?;
                let store = <C::BlockStoreInterface as BlockStoreInterface<C>>::init(
                    config.get::<C::BlockStoreInterface>(),
                )
                .expect("Could not init blockstore");

                let mut block = vec![0u8; 256 * 1025];

                'outer: for path in input {
                    let Ok(mut file) = fs::File::open(path) else {
                        log::error!("Could not open the file {path:?}");
                        continue;
                    };

                    let mut putter = store.put(None);

                    loop {
                        let Ok(size) = file.read(&mut block) else {
                            log::error!("read error");
                            break 'outer;
                        };

                        if size == 0 {
                            break;
                        }

                        putter
                            .write(
                                &block[0..size],
                                lightning_types::CompressionAlgorithm::Uncompressed,
                            )
                            .unwrap();
                    }

                    match putter.finalize().await {
                        Ok(hash) => {
                            println!("{:x}\t{path:?}", ByteBuf(&hash));
                        },
                        Err(e) => {
                            log::error!("Failed: {e}");
                        },
                    }
                }
                Ok(())
            },
        }
    }
}

#[test]
fn some() {
    let config = PathBuf::from("/config.toml");

    config.parent().unwrap();
}

struct ByteBuf<'a>(&'a [u8]);

impl<'a> std::fmt::LowerHex for ByteBuf<'a> {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        for byte in self.0 {
            fmtr.write_fmt(format_args!("{:02x}", byte))?;
        }
        Ok(())
    }
}
