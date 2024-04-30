use anyhow::Context;

use crate::connection::Connection;
use crate::header::{HttpOverrides, HttpResponse};

/// Respond with an error, utilizing the error code if the connection is http
#[inline(always)]
pub async fn respond_with_error(
    connection: &mut Connection,
    error: &[u8],
    status_code: u16,
) -> anyhow::Result<()> {
    if connection.is_http_request() {
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

/// Respond with a custom [`HttpResponse`] directly.
/// Panics in debug mode if the connection is not http.
#[inline(always)]
pub async fn respond_with_http_response(
    connection: &mut Connection,
    response: HttpResponse,
) -> anyhow::Result<()> {
    debug_assert!(connection.is_http_request());

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

/// Respond with some bytes, setting default headers if the connection is http.
#[inline(always)]
pub async fn respond(connection: &mut Connection, response: &[u8]) -> anyhow::Result<()> {
    respond_only_default_headers(connection).await?;

    // Send the body back now
    connection
        .write_payload(response)
        .await
        .context("failed to send error message")?;

    Ok(())
}

/// Send only the default headers, allowing for data to be streamed or sent directly afterwards.
#[inline(always)]
pub async fn respond_only_default_headers(connection: &mut Connection) -> anyhow::Result<()> {
    debug_assert!(connection.is_http_request());

    let header_bytes =
        serde_json::to_vec(&HttpOverrides::default()).context("Failed to serializez headers")?;

    // response with the headers first
    connection
        .write_payload(&header_bytes)
        .await
        .context("failed to send error headers")?;

    Ok(())
}
