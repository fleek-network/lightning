use anyhow::Context;
use base64::Engine;
use fn_sdk::connection::Connection;

use crate::libtorch;
use crate::libtorch::IMAGENET_CLASS_COUNT;
use crate::model::Model;
use crate::stream::ServiceStream;
use crate::task::{RunOpts, Task};

pub async fn handle(connection: Connection) -> anyhow::Result<()> {
    let mut stream = ServiceStream::new(connection);

    while let Some(request) = stream.recv().await {
        let device = request.device;
        match request.task {
            Task::Run { .. } => {
                let opts: RunOpts = serde_json::from_slice(&request.opts.as_bytes())
                    .context("invalid encoded opts")?;
                let input = base64::prelude::BASE64_STANDARD.decode(opts.input)?;
                let result = match opts.model.name.parse::<Model>()? {
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
            Task::Train { .. } => {
                unimplemented!()
            },
        }
    }

    Ok(())
}
