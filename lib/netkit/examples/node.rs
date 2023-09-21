use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use clap::{Parser, Subcommand};
use fleek_crypto::{NodePublicKey, NodeSecretKey, SecretKey};
use netkit::builder::Builder;
use netkit::endpoint::{Event, Message, NodeAddress, Request};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Pulse {
        #[arg(short, long)]
        key: Vec<String>,
        #[arg(short, long)]
        address: Vec<SocketAddr>,
        message: String,
    },
    Listen,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse();

    let sk = NodeSecretKey::generate();
    tracing::info!("public key {:?}", sk.to_pk().to_string());
    let mut endpoint = Builder::new(sk).build().unwrap();

    let request_tx = endpoint.request_sender();
    let (service_scope, mut event_rx) = endpoint.network_event_receiver();

    tokio::spawn(endpoint.start());

    match cli.command {
        Commands::Pulse {
            key: peer_key,
            address: peer_address,
            message,
        } => {
            if peer_key.len() != peer_address.len() {
                panic!("must pass peer key and peer address");
            }

            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        tracing::info!("sending message");
                        for (pk, socket_address) in peer_key.iter().zip(peer_address.iter()) {
                            let pk = NodePublicKey::from_str( pk).unwrap();
                            let address = NodeAddress {
                                pk,
                                socket_address: *socket_address,
                            };
                            let message = Message {
                                service: service_scope,
                                payload: message.as_bytes().to_vec()
                            };
                            request_tx.send(
                                Request::SendMessage {
                                    peer: address,
                                    message,
                                }
                            ).await
                            .unwrap();
                        }
                    }
                    event = event_rx.recv() => {
                        if event.is_none() {
                            break;
                        }
                        let event = event.unwrap();
                        match event {
                            Event::Message { peer, message } => {
                                tracing::info!("new message from {peer:?}: {message:?}");
                            }
                            Event::NewStream { peer, tx: _, rx: _ } => {
                                tracing::info!("new stream from {peer:?}");
                            }
                            Event::NewConnection { peer, rtt, .. } => {
                                tracing::info!("new connection from {peer:?} with {rtt:?}");
                            }
                            Event::Disconnect { peer } => {
                                tracing::info!("disconnected from {:?}", peer.to_string());
                            }
                        }
                    }
                }
            }
        },
        Commands::Listen => loop {
            tokio::select! {
                event = event_rx.recv() => {
                    if event.is_none() {
                        break;
                    }
                    let event = event.unwrap();
                    match event {
                        Event::Message { peer, message } => {
                            tracing::info!("new message from {:?}: {:?}", peer.to_string(), String::from_utf8(message.payload).unwrap());
                        }
                        Event::NewStream { peer, tx: _, rx: _ } => {
                            tracing::info!("new stream from {peer:?}");
                        }
                        Event::NewConnection { peer, rtt, .. } => {
                            tracing::info!("new connection from {:?} with rtt {rtt:?}", peer.to_string());
                        }
                        Event::Disconnect { peer } => {
                            tracing::info!("disconnected from {:?}", peer.to_string());
                        }
                    }
                }
            }
        },
    }
}
