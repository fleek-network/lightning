use std::collections::HashMap;
use std::fmt::Display;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Extension;
use base64::Engine;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use fleek_service_js_poc::stream::Origin;
use lightning_interfaces::ExecutorProviderInterface;
use lightning_schema::handshake::{HandshakeRequestFrame, RequestFrame};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::handshake::Context;
use crate::transports::http::{HttpReceiver, HttpSender};

pub async fn fetcher_service_handler<P: ExecutorProviderInterface>(
    Query(params): Query<HashMap<String, String>>,
    Extension(provider): Extension<Context<P>>,
) -> Result<Body, (StatusCode, String)> {
    let payload = match params.get("payload") {
        Some(payload) => base64::prelude::BASE64_URL_SAFE
            .decode(payload)
            .map_err(|_| bad_request("invalid payload"))?
            .into(),
        None => return Err(bad_request("missing payload")),
    };

    let handshake_frame = HandshakeRequestFrame::Handshake {
        service: 0,
        pk: ClientPublicKey([0; 96]),
        pop: ClientSignature([0; 48]),
        retry: None,
    };

    let (frame_tx, frame_rx) = async_channel::bounded(8);
    let (body_tx, body_rx) = async_channel::bounded(1024);
    let (termination_tx, termination_rx) = oneshot::channel();

    let sender = HttpSender {
        frame_tx,
        body_tx,
        expected_block_count: None,
        current_block_len: 0,
        termination_tx: Some(termination_tx),
    };
    let receiver = HttpReceiver { inner: frame_rx };
    let body = Body::from_stream(body_rx);

    sender
        .frame_tx
        .try_send(Some(RequestFrame::ServicePayload { bytes: payload }))
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "unexpected error".to_string(),
            )
        })?;

    provider
        .handle_new_connection(handshake_frame, sender, receiver)
        .await;

    // If there is an error while streaming, the status header has already been sent,
    // this is a hacky way of returning an error status before beginning streaming the body.
    match termination_rx.await {
        Ok(reason) => Err(bad_request(format!("handshake failed: {reason:?}"))),
        Err(_) => Ok(body),
    }
}

pub async fn js_service_handler<P: ExecutorProviderInterface>(
    Path(origin): Path<String>,
    Path(uri): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Extension(_): Extension<Context<P>>,
) -> Result<Body, (StatusCode, String)> {
    let origin = match origin.as_str() {
        "ipfs" => Origin::Ipfs,
        "blake3" => Origin::Blake3,
        _ => return Err(bad_request("unknown origin")),
    };

    let param = params
        .get("param")
        .map(|param| serde_json::from_str::<Value>(param).map_err(|_| bad_request("invalid param")))
        .transpose()?;

    let request = fleek_service_js_poc::stream::Request { origin, uri, param };

    Err(bad_request("failed"))
}

#[inline(always)]
fn bad_request<T: AsRef<str> + Display>(msg: T) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}
