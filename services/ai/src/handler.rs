use anyhow::{anyhow, Context};
use base64::Engine;
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
                train_data_uri,
                train_label_uri,
                validation_data_uri,
                validation_label_uri,
            } => {
                let result = handle_train_task(
                    epochs,
                    model_uri,
                    train_data_uri,
                    train_label_uri,
                    validation_data_uri,
                    validation_label_uri,
                )
                .await?;
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
    _epochs: u32,
    model_uri: String,
    train_data_uri: String,
    train_label_uri: String,
    validation_data_uri: String,
    validation_label_uri: String,
) -> anyhow::Result<TrainingResult> {
    let model_hash = get_hash(model_uri).await?;
    let train_data_hash = get_hash(train_data_uri).await?;
    let train_label_hash = get_hash(train_label_uri).await?;
    let validation_data_hash = get_hash(validation_data_uri).await?;
    let validation_label_hash = get_hash(validation_label_uri).await?;

    let model = fn_sdk::blockstore::ContentHandle::load(&model_hash)
        .await
        .context("failed to get handle for source from blockstore")?
        .read_to_end()
        .await
        .context("failed to read model from blockstore")?;
    let train_data = fn_sdk::blockstore::ContentHandle::load(&train_data_hash)
        .await
        .context("failed to get handle for source from blockstore")?
        .read_to_end()
        .await
        .context("failed to read train set from blockstore")?;
    let train_labels = fn_sdk::blockstore::ContentHandle::load(&train_label_hash)
        .await
        .context("failed to get handle for source from blockstore")?
        .read_to_end()
        .await
        .context("failed to read train set from blockstore")?;
    let validation_data = fn_sdk::blockstore::ContentHandle::load(&validation_data_hash)
        .await
        .context("failed to get handle for source from blockstore")?
        .read_to_end()
        .await
        .context("failed to read validation set from blockstore")?;
    let validation_labels = fn_sdk::blockstore::ContentHandle::load(&validation_label_hash)
        .await
        .context("failed to get handle for source from blockstore")?
        .read_to_end()
        .await
        .context("failed to read validation set from blockstore")?;

    train::train(
        Config {
            model: model.into(),
        },
        Dataset {
            train_images: train_data.into(),
            train_labels: train_labels.into(),
            validation_images: validation_data.into(),
            validation_labels: validation_labels.into(),
            labels: 10,
        },
    )
}
