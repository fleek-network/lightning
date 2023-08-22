use std::net::SocketAddr;

use fleek_cdn::client::CdnClient;
use fleek_cdn::connection::ServiceMode;
use fleek_crypto::ClientPublicKey;
use rand::rngs::ThreadRng;
use rand::Rng;
use tokio::net::TcpStream;

const HASH: [u8; 32] = [
    240, 76, 40, 117, 207, 118, 89, 141, 116, 76, 54, 143, 23, 169, 217, 135, 248, 10, 42, 172, 64,
    171, 193, 85, 186, 234, 102, 129, 48, 240, 126, 33,
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_addr: SocketAddr = ([0; 4], 6969).into();
    let mut pubkey = [0u8; 96];
    ThreadRng::default().fill(pubkey.as_mut_slice());
    let pubkey = ClientPublicKey(pubkey);
    println!("using pubkey: {pubkey}");

    // connect over tcp and setup client
    let mut tcp = TcpStream::connect(server_addr).await?;
    println!("connected to {server_addr}");
    let (r, w) = tcp.split();
    // setup client, performing a handshake and sending a service request to the server
    let mut client = CdnClient::new(r, w, pubkey).await.unwrap();

    // request some content
    let mut res = client.request(ServiceMode::Optimistic, HASH).await.unwrap();

    // iterate over each chunk of the data
    while let Some(bytes) = res.next().await? {
        println!("received verified chunk of data ({} bytes)", bytes.len());
    }

    Ok(())
}
