use std::{net::SocketAddr, time::Duration};

use clap::{Parser, Subcommand};
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSecretKey, SecretKey};
use lightning_application::query_runner::QueryRunner;
use lightning_dht::dht::{Builder, Dht};
use lightning_interfaces::{Blake3Hash, TopologyInterface, WithStartAndShutdown};
use lightning_topology::Topology;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, group = "bootstrap_address")]
    bootstrapper: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[group(required = true)]
enum Commands {
    Get {
        #[arg(short, long)]
        key: String,
    },
    Put,
    Join,
    Bootstrapper,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse();

    let bootstrap_key_pem = include_str!("../../test-utils/keys/test_network.pem");
    let bootstrap_secret_key = NodeNetworkingSecretKey::decode_pem(bootstrap_key_pem).unwrap();
    let bootstrap_key = bootstrap_secret_key.to_pk();

    match cli.command {
        Commands::Get { key } => {
            let address: SocketAddr = cli.bootstrapper.unwrap().parse().unwrap();
            let secret_key = NodeNetworkingSecretKey::generate();
            tracing::info!("public key: {:?}", secret_key.to_pk());
            let dht =
                start_node::<Topology<QueryRunner>>(secret_key, Some((address, bootstrap_key)))
                    .await;

            tracing::info!("GET {key:?}");

            let key = hex::decode(key).unwrap();
            if let Some(value) = dht.get(&key).await {
                tracing::info!("value found is {:?}", value.value);
            }
        },
        Commands::Put => {
            let address: SocketAddr = cli.bootstrapper.unwrap().parse().unwrap();
            let secret_key = NodeNetworkingSecretKey::generate();
            tracing::info!("public key: {:?}", secret_key.to_pk());
            let dht =
                start_node::<Topology<QueryRunner>>(secret_key, Some((address, bootstrap_key)))
                    .await;

            // Todo: get actual hash.
            let key: Blake3Hash = rand::random();
            let value: [u8; 4] = rand::random();

            tracing::info!("PUT {}:{value:?}", hex::encode(key));

            dht.put(&key, &value);

            // Todo: Let's remove this loop.
            // We have this loop so that the spawn task of `put` finishes.
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        },
        Commands::Join => {
            let address: SocketAddr = cli.bootstrapper.unwrap().parse().unwrap();
            let secret_key = NodeNetworkingSecretKey::generate();
            tracing::info!("public key: {:?}", secret_key.to_pk());
            let _ = start_node::<Topology<QueryRunner>>(secret_key, Some((address, bootstrap_key)))
                .await;
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        },
        Commands::Bootstrapper => {
            let _ = start_node::<Topology<QueryRunner>>(bootstrap_secret_key, None).await;
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        },
    }

    tracing::info!("shutting down dht-node");
}

async fn start_node<T: TopologyInterface>(
    secret_key: NodeNetworkingSecretKey,
    bootstrapper: Option<(SocketAddr, NodeNetworkingPublicKey)>,
) -> Dht<T> {
    let mut builder = Builder::new(secret_key);

    if let Some((address, key)) = bootstrapper {
        tracing::info!("bootstrapping to {address:?} {key:?}");
        builder.add_node(key, address);
    }

    let dht = builder.build().await.unwrap();
    dht.start().await;

    tracing::info!("start bootstrap");
    dht.bootstrap().await;

    while !dht.is_bootstrapped().await {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    tracing::info!("finished bootstrapping");

    dht
}
