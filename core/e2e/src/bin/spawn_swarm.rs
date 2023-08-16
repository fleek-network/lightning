use std::time::SystemTime;

use anyhow::Result;
use clap::Parser;
use fleek_crypto::PublicKey;
use lightning_e2e::{swarm::Swarm, utils::shutdown};
use resolved_pathbuf::ResolvedPathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Number of nodes to spawn
    #[arg(short, long, default_value_t = 4)]
    num_nodes: usize,

    /// Epoch duration in millis
    #[arg(short, long, default_value_t = 60000)]
    epoch_time: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/spawn-swarm").unwrap();
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_num_nodes(args.num_nodes)
        .with_epoch_time(args.epoch_time)
        .with_epoch_start(epoch_start)
        .build();
    swarm.launch().await.unwrap();

    let mut s = String::from("#####################################\n\n");
    for (pub_key, rpc_address) in swarm.get_rpc_addresses() {
        s.push_str(&format!(
            "BLS Public Key: {}\nRPC Address: {}\n\n",
            pub_key.to_base64(),
            rpc_address
        ));
    }
    s.push_str("#####################################");
    println!("{s}");

    shutdown::shutdown_stream().await;

    Ok(())
}
