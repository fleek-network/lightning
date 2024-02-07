use anyhow::{anyhow, Context};
use base64::Engine;
use fn_sdk::connection::Connection;

use crate::libtorch::train::{Config, Dataset, TrainingResult};
use crate::libtorch::{train, IMAGENET_CLASS_COUNT};
use crate::model::Model;
use crate::stream::ServiceStream;
use crate::task::{Run, Train};
use crate::{libtorch, Request};

pub async fn handle(connection: Connection) -> anyhow::Result<()> {
    let mut stream = ServiceStream::new(connection);
    while let Some(request) = stream.recv().await {
        match request {
            Request::Run(Run {
                model,
                input,
                device,
                ..
            }) => {
                match model.parse::<Model>() {
                    Ok(archived_model) => {
                        let input = base64::prelude::BASE64_STANDARD.decode(input)?;
                        let result = match archived_model {
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
                    Err(_) => {
                        let model = load_resource(&model).await?;
                        let output = libtorch::load_and_run_model(model.into(), input, device)?;
                        stream.send(output.as_ref()).await?;
                    },
                };
            },
            Request::Train(Train {
                model_uri,
                train_data_uri,
                train_label_uri,
                validation_data_uri,
                validation_label_uri,
                ..
            }) => {
                let result = handle_train_task(
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

async fn handle_train_task(
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
        .context("failed to get handle for model from blockstore")?
        .read_to_end()
        .await
        .context("failed to read model from blockstore")?;
    let train_data = fn_sdk::blockstore::ContentHandle::load(&train_data_hash)
        .await
        .context("failed to get handle for train data from blockstore")?
        .read_to_end()
        .await
        .context("failed to read train set from blockstore")?;
    let train_labels = fn_sdk::blockstore::ContentHandle::load(&train_label_hash)
        .await
        .context("failed to get handle for train labels from blockstore")?
        .read_to_end()
        .await
        .context("failed to read train set from blockstore")?;
    let validation_data = fn_sdk::blockstore::ContentHandle::load(&validation_data_hash)
        .await
        .context("failed to get handle for validation data from blockstore")?
        .read_to_end()
        .await
        .context("failed to read validation set from blockstore")?;
    let validation_labels = fn_sdk::blockstore::ContentHandle::load(&validation_label_hash)
        .await
        .context("failed to get handle for validation labels from blockstore")?
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
