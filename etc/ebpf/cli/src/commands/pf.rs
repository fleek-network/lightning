use std::net::SocketAddrV4;

use clap::Subcommand;
use ebpf_service::client::IpcSender;
use tokio::net::UnixStream;

#[derive(Debug, Subcommand)]
pub enum PfSubCmd {
    Block { addr: SocketAddrV4 },
    Allow { addr: SocketAddrV4 },
}

pub async fn block(address: SocketAddrV4) -> Result<(), anyhow::Error> {
    let stream = UnixStream::connect(".lightning/ebpf").await?;
    let mut client = IpcSender::new();
    client.init(stream);
    client.packet_filter_add(address).await?;
    Ok(())
}

pub async fn allow(address: SocketAddrV4) -> Result<(), anyhow::Error> {
    let stream = UnixStream::connect(".lightning/ebpf").await?;
    let mut client = IpcSender::new();
    client.init(stream);
    client.packet_filter_remove(address).await?;
    Ok(())
}
