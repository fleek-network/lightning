use std::collections::HashMap;
use std::fmt::Display;

use axum::body::Body;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::Extension;
use base64::Engine;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_interfaces::ExecutorProviderInterface;
use lightning_schema::handshake::{HandshakeRequestFrame, RequestFrame};
use tokio::sync::oneshot;

use crate::handshake::Context;
use crate::transports::http::{HttpReceiver, HttpSender};

pub async fn fetcher_service_handler<P: ExecutorProviderInterface>(
    Extension(provider): Extension<Context<P>>,
    Query(params): Query<HashMap<String, String>>,
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

pub fn js_service_handler<P: ExecutorProviderInterface>(
    Extension(provider): Extension<Context<P>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Body, (StatusCode, String)> {
    todo!()
}

#[inline(always)]
fn bad_request<T: AsRef<str> + Display>(msg: T) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}
