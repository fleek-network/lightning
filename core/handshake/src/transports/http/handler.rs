use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use axum::body::Body;
use axum::extract::{OriginalUri, Path, Query};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use bytes::Bytes;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use fn_sdk::header::{HttpMethod, HttpOverrides, TransportDetail};
use lightning_interfaces::schema::handshake::{HandshakeRequestFrame, RequestFrame};
use lightning_interfaces::ExecutorProviderInterface;
use lightning_metrics::increment_counter;
use tokio::sync::oneshot;
use url::Url;

use crate::handshake::Context;
use crate::transports::http::{HttpReceiver, HttpSender, Service};

pub async fn handler<P: ExecutorProviderInterface>(
    method: Method,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Path((service_id, _)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
    Extension(provider): Extension<Context<P>>,
    payload: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let service_id = u32::from_str(&service_id)
        .map_err(|_| (StatusCode::NOT_FOUND, "route not found".to_string()))
        .and_then(Service::try_from)?;

    let method = match method {
        Method::GET => HttpMethod::GET,
        Method::POST => HttpMethod::POST,
        Method::HEAD => HttpMethod::HEAD,
        Method::PUT => HttpMethod::PUT,
        Method::DELETE => HttpMethod::DELETE,
        Method::PATCH => HttpMethod::PATCH,
        _ => return Err((StatusCode::NOT_FOUND, "invalid method".to_string())),
    };

    let body_frame = RequestFrame::ServicePayload { bytes: payload };

    let handshake_frame = HandshakeRequestFrame::Handshake {
        service: service_id as u32,
        pk: ClientPublicKey([0; 96]),
        pop: ClientSignature([0; 48]),
        retry: None,
    };

    let (frame_tx, frame_rx) = async_channel::bounded(8);
    let (body_tx, body_rx) = async_channel::bounded(16);
    let (termination_tx, termination_rx) = oneshot::channel();

    // Create the url sent to the service
    let path = uri.path().split('/').skip(3).collect::<Vec<_>>().join("/");
    let mut url = Url::parse("http://fleek/").unwrap();
    url.set_path(&path);
    url.set_query(uri.query());

    let sender = HttpSender::new(service_id, frame_tx, body_tx, termination_tx);
    let receiver = HttpReceiver::new(
        frame_rx,
        TransportDetail::HttpRequest {
            method,
            url,
            header: headers
                .into_iter()
                .filter_map(|(name, val)| {
                    if let Some(name) = name {
                        if let Ok(val) = val.to_str() {
                            Some((name.to_string(), val.to_string()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect(),
        },
    );

    sender.frame_tx.try_send(Some(body_frame)).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected error".to_string(),
        )
    })?;

    {
        let service_id = format!("{}", service_id as usize);
        increment_counter!(
            "handshake_http_sessions",
            Some("Counter for number of handshake sessions accepted over http"),
            "service_id" => service_id.as_str()
        );
    }

    provider
        .handle_new_connection(handshake_frame, sender, receiver)
        .await;

    let mut response_builder = Response::builder();

    if let Some(content_type) = params.get("mime") {
        response_builder = response_builder.header("Content-Type", content_type);
    }

    // If the service is the javascript service. Await the first response as the possible header
    // overrides and over ride the response headers
    if matches!(service_id, Service::Js | Service::Fetcher) {
        let header_bytes = body_rx
            .recv()
            .await
            .map_err(|_| bad_request("Connection closed before headers were sent"))
            .and_then(|res| {
                res.map_err(|e| bad_request(format!("Unable to get headers from service: {e}")))
            })?;
        let header_overrides =
            serde_json::from_slice::<HttpOverrides>(&header_bytes).unwrap_or_default();

        if let Some(headers) = header_overrides.headers {
            for header in headers {
                for header_value in header.1 {
                    response_builder = response_builder.header(header.0.clone(), header_value);
                }
            }
        }
        if let Some(status) = header_overrides.status {
            response_builder = response_builder.status(status);
        }
    }

    let body = Body::from_stream(body_rx);

    // If there is an error while streaming, the status header has already been sent,
    // this is a hacky way of returning an error status before beginning streaming the body.
    match termination_rx.await {
        Ok(reason) => Err(bad_request(format!("handshake failed: {reason:?}"))),
        Err(_) => response_builder
            .body(body)
            .map_err(|_| bad_request("invalid type value")),
    }
}

#[inline(always)]
fn bad_request<T: AsRef<str> + Display>(msg: T) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}
