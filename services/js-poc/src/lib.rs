use std::sync::mpsc::TrySendError;

use anyhow::{bail, Context};
use deno_core::v8::{Global, IsolateHandle, Value};
use deno_core::{serde_v8, v8, JsRuntime, ModuleSpecifier};
use fn_sdk::connection::Connection;
use fn_sdk::header::TransportDetail;
use fn_sdk::http_util::{respond, respond_with_error, respond_with_http_response};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info, warn};

use crate::runtime::Runtime;
use crate::stream::{Origin, Request};

mod http;
mod runtime;
pub mod stream;

pub(crate) mod params {
    use std::time::Duration;

    pub const HEAP_INIT: usize = 1 << 10;
    pub const HEAP_LIMIT: usize = 50 << 20;
    pub const REQ_TIMEOUT: Duration = Duration::from_secs(15);
    pub const FETCH_BLACKLIST: &[&str] = &["localhost", "127.0.0.1", "::1"];
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    fn_sdk::ipc::init_from_env();

    info!("Initialized POC JS service!");

    let mut listener = fn_sdk::ipc::conn_bind().await;

    // Explicitly initialize the v8 platform on the main thread
    JsRuntime::init_platform(None);

    // Initialize node polyfill imports
    runtime::module_loader::get_or_init_imports();

    // To cancel events mid execution.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<IsolateHandle>();
    tokio::spawn(async move {
        while let Some(handle) = rx.recv().await {
            tokio::spawn(async move {
                tokio::time::sleep(params::REQ_TIMEOUT).await;
                handle.terminate_execution();
            });
        }
    });

    let mut pool = Vec::new();
    for _ in 0..num_cpus::get_physical() {
        let (job_tx, job_rx) = std::sync::mpsc::sync_channel(0);
        pool.push(job_tx);

        // TODO: Research using deno's JsRealms to provide the script sandboxing in a single or a
        // few shared multithreaded runtimes, or use a custom work scheduler.
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create connection async runtime");
            while let Ok((tx, conn)) = job_rx.recv() {
                if let Err(e) = runtime.block_on(handle_connection(tx, conn)) {
                    error!("session failed: {e:?}");
                }
            }
        });
    }

    while let Ok(conn) = listener.accept().await {
        let tx = tx.clone();
        let mut job_params = Some((tx, conn));
        'outer: loop {
            println!("looping");
            for job_tx in &pool {
                let (tx, conn) = job_params.expect("");
                match job_tx.try_send((tx, conn)) {
                    Ok(_) => break 'outer,
                    Err(TrySendError::Full((tx, conn))) => job_params = Some((tx, conn)),
                    Err(TrySendError::Disconnected((tx, conn))) => {
                        warn!("a thread in the pool disconnected");
                        job_params = Some((tx, conn));
                    },
                };
            }
        }
    }
}

async fn handle_connection(
    tx: UnboundedSender<IsolateHandle>,
    mut connection: Connection,
) -> anyhow::Result<()> {
    match &connection.header.transport_detail {
        TransportDetail::HttpRequest { .. } => {
            let body = connection
                .read_payload()
                .await
                .context("Could not read body.")?;

            let TransportDetail::HttpRequest {
                method,
                ref url,
                ref header,
            } = connection.header.transport_detail
            else {
                unreachable!()
            };
            let request = http::request::extract(url, header, method, body.to_vec())
                .context("failed to parse request")?;

            if let Err(e) = handle_request(0, &mut connection, &tx, request).await {
                respond_with_error(&mut connection, format!("{e:?}").as_bytes(), 400).await?;
                return Err(e);
            }
        },
        TransportDetail::Task { depth, payload } => {
            let request: Request = serde_json::from_slice(payload)?;
            if let Err(e) = handle_request(*depth, &mut connection, &tx, request).await {
                respond_with_error(&mut connection, e.to_string().as_bytes(), 400).await?;
                return Err(e);
            }
        },
        TransportDetail::Other => {
            while let Some(payload) = connection.read_payload().await {
                let request: Request = serde_json::from_slice(&payload)?;
                if let Err(e) = handle_request(0, &mut connection, &tx, request).await {
                    respond_with_error(&mut connection, e.to_string().as_bytes(), 400).await?;
                    return Err(e);
                };
            }
        },
    }

    Ok(())
}

async fn handle_request(
    depth: u8,
    connection: &mut Connection,
    tx: &UnboundedSender<IsolateHandle>,
    request: Request,
) -> anyhow::Result<()> {
    let Request {
        origin,
        uri,
        path,
        param,
    } = request;
    if uri.is_empty() {
        bail!("Empty origin uri");
    }

    let module_url = match origin {
        Origin::Blake3 => format!("blake3://{uri}"),
        Origin::Ipfs => format!("ipfs://{uri}"),
        Origin::Http => uri,
        Origin::Unknown => todo!(),
    }
    .parse::<ModuleSpecifier>()
    .context("Invalid origin URI")?;

    let mut location = module_url.clone();
    if let Some(path) = path {
        location = location.join(&path).context("Invalid path string")?;
    }

    // Create runtime and execute the source
    let mut runtime =
        Runtime::new(location.clone(), depth).context("Failed to initialize runtime")?;
    tx.send(runtime.deno.v8_isolate().thread_safe_handle())
        .context("Failed to send the IsolateHandle to main thread.")?;

    let res = match runtime.exec(&module_url, param).await? {
        Some(res) => res,
        None => {
            bail!("No response available");
        },
    };

    // Resolve async if applicable
    // TODO: figure out why `deno.resolve` doesn't drive async functions
    #[allow(deprecated)]
    let res = tokio::time::timeout(params::REQ_TIMEOUT, runtime.deno.resolve_value(res))
        .await
        .context("Execution timeout")??;

    parse_and_respond(connection, &mut runtime, res).await?;

    let feed = runtime.end();
    debug!("{feed:?}");

    Ok(())
}

async fn parse_and_respond(
    connection: &mut Connection,
    runtime: &mut Runtime,
    res: Global<Value>,
) -> anyhow::Result<()> {
    // Handle the return data
    let scope = &mut runtime.deno.handle_scope();
    let local = v8::Local::new(scope, res);

    if local.is_uint8_array() || local.is_array_buffer() {
        // If the return type is a U8 array, send the raw data directly to the client
        let bytes = match deno_core::_ops::to_v8_slice_any(local) {
            Ok(slice) => slice.to_vec(),
            Err(e) => bail!("failed to parse bytes: {e}"),
        };
        respond(connection, &bytes).await?;
    } else if local.is_string() {
        // Likewise for string types
        let string = serde_v8::from_v8::<String>(scope, local)
            .context("failed to deserialize response string")?;

        respond(connection, string.as_bytes()).await?;
    } else {
        // Parse the response into a generic json value
        let value = serde_v8::from_v8::<serde_json::Value>(scope, local)
            .context("failed to deserialize response")?
            .clone();

        // Attempt to parse and use the value as an http response override object
        if connection.is_http_request() {
            if let Ok(http_response) = http::response::parse(&value) {
                respond_with_http_response(connection, http_response).await?;
                return Ok(());
            }
        }

        // Otherwise, send the data as a json string
        let res = serde_json::to_string(&value).context("failed to encode json response")?;
        respond(connection, res.as_bytes()).await?;
    }

    Ok(())
}
