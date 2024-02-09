use anyhow::{anyhow, bail, Context};
use bytes::Bytes;
use fn_sdk::connection::Connection;
use fn_sdk::header::TransportDetail;
use url::Url;

use crate::libtorch::inference;
use crate::{Device, Infer, Origin, Request, Train};

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
        let model = load_resource(&uri).await?;
        let output = inference::load_and_run_model(model.into(), body.into(), Device::Cpu)?;
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
                ..
            }) => {
                match handle_inference_task(model, origin, input, device).await {
                    Ok(output) => connection.write_payload(output.as_ref()).await?,
                    Err(e) => {
                        // Todo: let's define a better API to communicate success vs error.
                        connection.write_payload(e.to_string().as_bytes()).await?;
                    },
                }
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
) -> anyhow::Result<Bytes> {
    let hash = get_hash(origin, &model).await?;
    let model = fn_sdk::blockstore::ContentHandle::load(&hash)
        .await
        .context("failed to get resource from blockstore")?
        .read_to_end()
        .await?;
    inference::load_and_run_model(model.into(), input, device)
}

#[inline(always)]
async fn get_hash(origin: Origin, uri: &str) -> anyhow::Result<[u8; 32]> {
    match origin {
        Origin::Blake3 => {
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
        },
        Origin::Ipfs => fn_sdk::api::fetch_from_origin(fn_sdk::api::Origin::IPFS, uri.as_bytes())
            .await
            .ok_or(anyhow::anyhow!("failed to get hash from IPFS origin")),
        Origin::Http => {
            anyhow::bail!("HTTP origin is not supported");
        },
        Origin::Unknown => {
            anyhow::bail!("unknown origin");
        },
    }
}
