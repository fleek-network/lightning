use std::net::SocketAddr;

use anyhow::Context;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use lightning_interfaces::ShutdownWaiter;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::endpoint::EndpointTask;
use crate::event::Event;
use crate::state::State;

pub async fn spawn_http_server(
    addr: SocketAddr,
    shutdown: ShutdownWaiter,
    event_tx: Sender<Event>,
    endpoint_task_tx: Sender<EndpointTask>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/state", axum::routing::get(state))
        .layer(Extension(event_tx))
        .layer(Extension(endpoint_task_tx));
    let shutdown = async move {
        shutdown.wait_for_shutdown().await;
    };
    let listener = TcpListener::bind(&addr).await?;
    axum::serve::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown)
        .await
        .context("failed to run http server")
}

async fn state(
    Extension(event_tx): Extension<Sender<Event>>,
    Extension(_): Extension<Sender<EndpointTask>>,
) -> Result<Response, StatusCode> {
    let (respond_tx, respond_rx) = oneshot::channel();
    if event_tx
        .send(Event::GetStats {
            respond: respond_tx,
        })
        .await
        .is_ok()
    {
        let receiver_info = respond_rx
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let state = State {
            connections: receiver_info.connections,
            event_queue_cap: event_tx.capacity(),
            event_queue_max_cap: event_tx.max_capacity(),
            endpoint_task_queue_cap: receiver_info.endpoint_queue_cap,
            endpoint_task_queue_max_cap: receiver_info.endpoint_queue_max_cap,
            ongoing_endpoint_async_tasks: receiver_info.ongoing_endpoint_async_tasks,
        };

        let value = serde_json::to_string(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(value.into_response())
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
