use std::net::SocketAddrV4;

use clap::Subcommand;
use ebpf_service::filter::PacketFilter;
use tokio::net::UnixStream;

#[derive(Debug, Subcommand)]
pub enum PfSubCmd {
    Block { addr: SocketAddrV4 },
    Allow { addr: SocketAddrV4 },
}

pub async fn block(address: SocketAddrV4) -> Result<(), anyhow::Error> {
    let stream = UnixStream::connect(".lightning/ebpf").await?;
    let mut client = PacketFilter::new();
    client.init(stream);
    client.add(address).await?;
    Ok(())
}

pub async fn allow(address: SocketAddrV4) -> Result<(), anyhow::Error> {
    let stream = UnixStream::connect(".lightning/ebpf").await?;
    let mut client = PacketFilter::new();
    client.init(stream);
    client.remove(address).await?;
    Ok(())
}
