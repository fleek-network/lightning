use anyhow::{anyhow, Context};
use fn_sdk::connection::Connection;

use crate::libtorch::inference;
use crate::stream::ServiceStream;
use crate::{Infer, Request, Train};

pub async fn handle(connection: Connection) -> anyhow::Result<()> {
    let mut stream = ServiceStream::new(connection);
    while let Some(request) = stream.recv().await {
        match request {
            Request::Infer(Infer {
                model,
                input,
                device,
                ..
            }) => {
                let model = load_resource(&model).await?;
                let output = inference::load_and_run_model(model.into(), input, device)?;
                stream.send(output.as_ref()).await?;
            },
            Request::Train(Train { .. }) => {
                unimplemented!("under construction");
            },
        }
    }

    Ok(())
}

async fn load_resource(uri: &str) -> anyhow::Result<Vec<u8>> {
    // Todo: update param to accept &str.
    let hash = get_hash(uri.to_string()).await?;
    fn_sdk::blockstore::ContentHandle::load(&hash)
        .await
        .context("failed to get resource from blockstore")?
        .read_to_end()
        .await
        .map_err(Into::into)
}

async fn get_hash(uri: String) -> anyhow::Result<[u8; 32]> {
    // Todo: handle different origin types.
    let hash = hex::decode(uri).context("failed to decode blake3 hash")?;
    if hash.len() != 32 {
        return Err(anyhow!("invalid blake3 hash length"));
    }

    let hash: [u8; 32] = hash
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid hash"))?;

    if fn_sdk::api::fetch_blake3(hash).await {
        Ok(hash)
    } else {
        Err(anyhow!("failed to fetch file"))
    }
}
