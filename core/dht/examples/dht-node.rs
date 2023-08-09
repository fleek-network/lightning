use std::{net::SocketAddr, time::Duration};

use clap::{Parser, Subcommand};
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_dht::dht::{Builder, Dht};
use lightning_interfaces::Blake3Hash;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Get {
        #[arg(short, long)]
        key: String,
        bootstrapper: String,
    },
    Put {
        bootstrapper: String,
    },
    Bootstrapper,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Get { key, bootstrapper } => {
            let address: SocketAddr = bootstrapper.parse().unwrap();
            let dht = start_node(Some(address)).await;

            tracing::info!("GET {key:?}");

            let key = key.as_bytes();
            if let Some(value) = dht.get(key).await {
                tracing::info!("value found is {:?}", value.value);
            }
        },
        Commands::Put { bootstrapper } => {
            let address: SocketAddr = bootstrapper.parse().unwrap();
            let dht = start_node(Some(address)).await;

            // Todo: get actual hash.
            let key: Blake3Hash = rand::random();
            let value: [u8; 4] = rand::random();

            tracing::info!("generated key {key:?} and value {value:?}");

            dht.put(&key, &value);
        },
        Commands::Bootstrapper => {
            let _ = start_node(None).await;
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        },
    }

    tracing::info!("shutting down dht-node");
}

async fn start_node(bootstrapper: Option<SocketAddr>) -> Dht {
    let mut builder = Builder::new();

    let public_key = NodeNetworkingPublicKey(rand::random());
    tracing::info!("public key: {public_key:?}");

    if let Some(address) = bootstrapper {
        builder.set_address(address);
    }

    builder.set_node_key(public_key);

    let dht = builder.build().await.unwrap();

    tracing::info!("start bootstrap");
    dht.bootstrap().await;

    while !dht.is_bootstrapped().await {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    tracing::info!("finished bootstrapping");

    dht
}
