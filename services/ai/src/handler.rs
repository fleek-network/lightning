use anyhow::{anyhow, Context};
use base64::Engine;
use bytes::Bytes;
use fn_sdk::connection::Connection;

use crate::libtorch;
use crate::libtorch::train::{Config, Dataset, TrainingResult};
use crate::libtorch::{train, IMAGENET_CLASS_COUNT};
use crate::model::Model;
use crate::stream::ServiceStream;
use crate::task::Task;

pub async fn handle(connection: Connection) -> anyhow::Result<()> {
    let mut stream = ServiceStream::new(connection);

    while let Some(request) = stream.recv().await {
        let device = request.device;
        match request.task {
            Task::Run { model, input, .. } => {
                let input = base64::prelude::BASE64_STANDARD.decode(input)?;
                let result = match model.name.parse::<Model>()? {
                    Model::Resnet18 => {
                        libtorch::run_resnet18(&input, device, IMAGENET_CLASS_COUNT)?
                    },
                    Model::Resnet34 => {
                        libtorch::run_resnet34(&input, device, IMAGENET_CLASS_COUNT)?
                    },
                };
                let json_str = serde_json::to_string(&result.first())?;
                stream.send(json_str.as_bytes()).await?;
            },
            Task::Train {
                epochs,
                model_uri,
                train_set_uri,
                ..
            } => {
                let result = handle_train_task(epochs, model_uri, train_set_uri).await?;
                stream
                    .send(serde_json::to_string(&result)?.as_bytes())
                    .await?;
            },
        }
    }

    Ok(())
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
        return Err(anyhow!("failed to fetch file"));
    }
}

async fn handle_train_task(
    epochs: u32,
    model_uri: String,

    train_set_uri: String,
) -> anyhow::Result<TrainingResult> {
    // let _ = get_hash(model_uri).await?;
    // let _ = get_hash(train_set_uri).await?;
    //
    // let model = fn_sdk::blockstore::ContentHandle::load(&model_hash)
    //     .await
    //     .context("failed to get handle for source from blockstore")?
    //     .read_to_end()
    //     .await
    //     .context("failed to read source from blockstore")?;
    // let mut model =
    //     String::from_utf8(model).context("failed to parse source as utf8")?;
    //
    // let train_set = fn_sdk::blockstore::ContentHandle::load(&train_set_hash)
    //     .await
    //     .context("failed to get handle for source from blockstore")?
    //     .read_to_end()
    //     .await
    //     .context("failed to read source from blockstore")?;

    train::train(
        Config {
            model: Bytes::new(),
        },
        Dataset {},
    )
}
