use anyhow::{anyhow, bail, Context};
use bytes::Bytes;
use fn_sdk::connection::Connection;
use fn_sdk::header::TransportDetail;
use url::Url;

use crate::runtime::Session;
use crate::{Origin, StartSession};

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

        // Load model.
        let model = load_model(uri, origin).await?;
        let session = Session::new(model)?;

        // Run inference.
        let output = session.run(body.into())?;
        let serialized_output = rmp_serde::to_vec_named(&output)?;

        connection.write_payload(&serialized_output).await?;

        return Ok(());
    }

    // Read the header.
    let Some(initial_message) = connection.read_payload().await else {
        return Ok(());
    };

    let message = rmp_serde::from_slice::<StartSession>(initial_message.as_ref())
        .context("Could not deserialize initial message")?;

    // Load model.
    let model = load_model(message.model, message.origin).await?;
    let session = Session::new(model)?;

    // Process incoming inference requests.
    while let Some(payload) = connection.read_payload().await {
        let output = session.run(payload.freeze())?;
        let serialized_output = rmp_serde::to_vec_named(&output)?;
        connection.write_payload(&serialized_output).await?;
    }

    Ok(())
}

fn parse_http_url(url: &Url) -> Option<(Origin, String)> {
    let mut segments = url.path_segments()?;
    let seg1 = segments.next()?;
    let seg2 = segments.next()?;
    Some((seg1.into(), seg2.into()))
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
