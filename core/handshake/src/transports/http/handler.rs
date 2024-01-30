use std::collections::HashMap;
use std::fmt::Display;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Extension;
use base64::Engine;
use cid::Cid;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use fleek_service_js_poc::stream::{Origin, Request};
use lightning_interfaces::ExecutorProviderInterface;
use lightning_schema::handshake::{HandshakeRequestFrame, RequestFrame};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::handshake::Context;
use crate::transports::http::{HttpReceiver, HttpSender, Service};

pub async fn fetcher_service_handler<P: ExecutorProviderInterface>(
    Path((origin, uri)): Path<(String, String)>,
    Extension(provider): Extension<Context<P>>,
) -> Result<Body, (StatusCode, String)> {
    let origin = match origin.as_str() {
        "ipfs" => Origin::Ipfs,
        "blake3" => Origin::Blake3,
        _ => return Err(bad_request("unknown origin")),
    };
    let uri = match origin {
        Origin::Blake3 => base64::prelude::BASE64_URL_SAFE
            .decode(uri)
            .map_err(|_| bad_request("invalid uri value"))?,
        Origin::Ipfs => Cid::try_from(uri)
            .map_err(|_| bad_request("invalid uri value"))?
            .into(),
        Origin::Unknown => unreachable!(),
    };

    let mut payload = Vec::with_capacity(1 + uri.len());
    payload.push(origin as u8);
    payload.extend(uri);

    let request_frame = RequestFrame::ServicePayload {
        bytes: payload.into(),
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

    let sender = HttpSender::new(Service::Fetcher, frame_tx, body_tx, termination_tx);
    let receiver = HttpReceiver::new(frame_rx);
    let body = Body::from_stream(body_rx);

    sender.frame_tx.try_send(Some(request_frame)).map_err(|_| {
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
    Path((origin, uri)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
    Extension(provider): Extension<Context<P>>,
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

    let request_frame = RequestFrame::ServicePayload {
        bytes: serde_json::to_string(&Request { origin, uri, param })
            .map_err(|_| bad_request("failed to encode request"))?
            .into(),
    };
    let handshake_frame = HandshakeRequestFrame::Handshake {
        service: 1,
        pk: ClientPublicKey([0; 96]),
        pop: ClientSignature([0; 48]),
        retry: None,
    };

    let (frame_tx, frame_rx) = async_channel::bounded(8);
    let (body_tx, body_rx) = async_channel::bounded(1024);
    let (termination_tx, termination_rx) = oneshot::channel();

    let sender = HttpSender::new(Service::Js, frame_tx, body_tx, termination_tx);
    let receiver = HttpReceiver::new(frame_rx);
    let body = Body::from_stream(body_rx);

    sender.frame_tx.try_send(Some(request_frame)).map_err(|_| {
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

#[inline(always)]
fn bad_request<T: AsRef<str> + Display>(msg: T) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}
