use fleek_ipld::dag_pb::download;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    download().await
}
