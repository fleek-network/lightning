use anyhow::{anyhow, bail, Context};
use bytes::Bytes;
use fn_sdk::connection::Connection;
use fn_sdk::header::TransportDetail;
use url::Url;

use crate::backend::{libtorch, onnx};
use crate::{Backend, Device, Infer, Origin, Request, Train};

pub async fn handle(mut connection: Connection) -> anyhow::Result<()> {
    if connection.is_http_request() {
        let TransportDetail::HttpRequest { uri, .. } = &connection.header.transport_detail else {
            unreachable!()
        };

        let Some((origin, uri)) = parse_http_url(uri) else {
            let _ = connection.write_payload(b"invalid request url").await;
            bail!("Invalid url");
        };

        if origin != Origin::Blake3 {
            let _ = connection.write_payload(b"unsupported origin").await;
            bail!("unsupported origin");
        }

        let body = connection
            .read_payload()
            .await
            .context("Could not read body")?;

        // Todo: close connection properly.
        let output =
            handle_inference_task(uri, origin, body.into(), Device::Cpu, Backend::Onnx).await?;
        connection.write_payload(output.as_ref()).await?;

        return Ok(());
    }

    while let Some(payload) = connection.read_payload().await {
        let request: Request =
            serde_json::from_slice(&payload).context("Could not deserialize payload")?;
        match request {
            Request::Infer(Infer {
                model,
                input,
                device,
                origin,
                backend,
                ..
            }) => {
                // Todo: Let's define a better API to communicate success vs failure.
                let output = handle_inference_task(model, origin, input, device, backend).await?;
                connection.write_payload(output.as_ref()).await?;
            },
            Request::Train(Train { .. }) => {
                bail!("under construction");
            },
        }
    }

    Ok(())
}

fn parse_http_url(url: &Url) -> Option<(Origin, String)> {
    let mut segments = url.path_segments()?;
    let seg1 = segments.next()?;
    let seg2 = segments.next()?;
    Some((seg1.into(), seg2.into()))
}

#[inline(always)]
async fn handle_inference_task(
    model: String,
    origin: Origin,
    input: Bytes,
    device: Device,
    backend: Backend,
) -> anyhow::Result<Bytes> {
    let hash = match origin {
        Origin::Blake3 => {
            let hash = hex::decode(model).context("failed to decode blake3 hash")?;
            if hash.len() != 32 {
                return Err(anyhow!("invalid blake3 hash length"));
            }

            let hash: [u8; 32] = hash.try_into().map_err(|_| anyhow!("invalid hash"))?;

            if fn_sdk::api::fetch_blake3(hash).await {
                hash
            } else {
                bail!("failed to fetch file")
            }
        },
        Origin::Ipfs => fn_sdk::api::fetch_from_origin(fn_sdk::api::Origin::IPFS, model.as_bytes())
            .await
            .ok_or(anyhow!("failed to get hash from IPFS origin"))?,
        Origin::Http => {
            bail!("HTTP origin is not supported");
        },
        Origin::Unknown => {
            bail!("unknown origin");
        },
    };

    let model = fn_sdk::blockstore::ContentHandle::load(&hash)
        .await
        .context("failed to get resource from blockstore")?
        .read_to_end()
        .await?;

    match backend {
        Backend::LibTorch => libtorch::inference::load_and_run_model(model.into(), input, device),
        Backend::Onnx => onnx::inference::load_and_run_model(model.into(), input, device),
    }
}
