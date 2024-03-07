use std::borrow::Cow;

use anyhow::{anyhow, bail, Context};
use base64::Engine;
use bytes::Bytes;
use fn_sdk::connection::Connection;
use fn_sdk::header::TransportDetail;
use url::Url;

use crate::opts::{Encoding, Format};
use crate::runtime::{RunOutput, Session};
use crate::{Origin, StartSession};

pub async fn handle(mut connection: Connection) -> anyhow::Result<()> {
    if connection.is_http_request() {
        let TransportDetail::HttpRequest { uri, .. } = &connection.header.transport_detail else {
            unreachable!()
        };

        let (content_format, model_io_encoding) = parse_query_params(uri)?;

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

        // Load model.
        let model = load_model(uri, origin).await?;
        let session = Session::new(model, model_io_encoding)?;

        // Run inference.
        let output = serialize_output(session.run(body.freeze())?, &content_format)?;
        connection.write_payload(&output).await?;

        return Ok(());
    }

    // Read the header.
    let Some(initial_message) = connection.read_payload().await else {
        return Ok(());
    };

    let session_params = serde_json::from_slice::<StartSession>(initial_message.as_ref())
        .context("Could not deserialize initial message")?;

    // Load model.
    let model = load_model(session_params.model, session_params.origin).await?;
    let session = Session::new(model, session_params.model_io_encoding)?;

    // Process incoming inference requests.
    while let Some(payload) = connection.read_payload().await {
        let output = serialize_output(
            session.run(payload.freeze())?,
            &session_params.content_format,
        )?;
        connection.write_payload(&output).await?;
    }

    Ok(())
}

fn parse_http_url(url: &Url) -> Option<(Origin, String)> {
    let mut segments = url.path_segments()?;
    let seg1 = segments.next()?;
    let seg2 = segments.next()?;
    Some((seg1.into(), seg2.into()))
}

fn parse_query_params(url: &Url) -> anyhow::Result<(Format, Encoding)> {
    let mut content_format = None;
    let mut encoding = None;
    for (name, value) in url.query_pairs() {
        if name == "format" {
            content_format = Some(value);
        } else if name == "encoding" {
            encoding = Some(value);
        }

        if content_format.is_some() && encoding.is_some() {
            break;
        }
    }
    Ok((
        content_format.unwrap_or(Cow::Borrowed("json")).parse()?,
        encoding.unwrap_or(Cow::Borrowed("borsh")).parse()?,
    ))
}

async fn load_model(model: String, origin: Origin) -> anyhow::Result<Bytes> {
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
            // This should be unreachable because we're skipping it during deserialization.
            bail!("unknown origin");
        },
    };

    let model = fn_sdk::blockstore::ContentHandle::load(&hash)
        .await
        .context("failed to get resource from blockstore")?
        .read_to_end()
        .await?;

    Ok(model.into())
}

fn serialize_output(output: RunOutput, format: &Format) -> anyhow::Result<Bytes> {
    let output = match output {
        RunOutput::SafeTensors(bytes) => {
            if format.is_json() {
                serde_json::to_string(&serde_json::json!({
                    "safetensors": base64::prelude::BASE64_STANDARD.encode(bytes),
                }))?
                .into_bytes()
                .into()
            } else {
                bytes
            }
        },
        RunOutput::Borsh(named_vectors) => {
            // Todo: which binary encoding could we support?
            serde_json::to_string(&named_vectors)?.into_bytes().into()
        },
    };

    Ok(output)
}
