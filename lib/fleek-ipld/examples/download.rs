use fleek_ipld::dag_cbor::download;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    download().await
}
