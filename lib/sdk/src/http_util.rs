use anyhow::Context;

use crate::connection::Connection;
use crate::header::{HttpOverrides, HttpResponse};

pub async fn respond_with_error(
    connection: &mut Connection,
    error: &[u8],
    status_code: u16,
    is_http: bool,
) -> anyhow::Result<()> {
    if is_http {
        let headers = HttpOverrides {
            status: Some(status_code),
            headers: None,
        };
        let header_bytes = serde_json::to_vec(&headers).context("Failed to serialize headers")?;

        // respond with the headers first
        connection
            .write_payload(&header_bytes)
            .await
            .context("failed to send error headers")?;
    }
    // Send the error message back now as body
    connection
        .write_payload(error)
        .await
        .context("failed to send error message")?;

    Ok(())
}

pub async fn respond_to_client_with_http_response(
    connection: &mut Connection,
    response: HttpResponse,
) -> anyhow::Result<()> {
    let headers = HttpOverrides {
        status: response.status,
        headers: response.headers,
    };
    let header_bytes = serde_json::to_vec(&headers).context("Failed to serialize headers")?;

    // respond with headers first
    connection
        .write_payload(&header_bytes)
        .await
        .context("failed to send error headers")?;

    // send body back
    connection
        .write_payload(response.body.as_bytes())
        .await
        .context("failed to send body")?;

    Ok(())
}

pub async fn respond_to_client(
    connection: &mut Connection,
    response: &[u8],
    is_http: bool,
) -> anyhow::Result<()> {
    if is_http {
        let header_bytes = serde_json::to_vec(&HttpOverrides::default())
            .context("Failed to serializez headers")?;

        // response with the headers first
        connection
            .write_payload(&header_bytes)
            .await
            .context("failed to send error headers")?;
    }
    // Send the body back now
    connection
        .write_payload(response)
        .await
        .context("failed to send error message")?;

    Ok(())
}

pub async fn respond_to_client_with_http_headers(
    connection: &mut Connection,
    is_http: bool,
) -> anyhow::Result<()> {
    if is_http {
        let header_bytes = serde_json::to_vec(&HttpOverrides::default())
            .context("Failed to serializez headers")?;

        // response with the headers first
        connection
            .write_payload(&header_bytes)
            .await
            .context("failed to send error headers")?;
    }
    Ok(())
}
