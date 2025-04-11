use anyhow::{bail, Context};
use cid::Cid;
use config::FleekConfig;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_core::v8::{Global, IsolateHandle, Value};
use deno_core::{serde_v8, v8, JsRuntime, ModuleSpecifier};
use fn_sdk::blockstore::b3fs::entry::BorrowedLink;
use fn_sdk::blockstore::ContentHandle;
use fn_sdk::connection::Connection;
use fn_sdk::header::TransportDetail;
use fn_sdk::http_util::{respond, respond_with_error, respond_with_http_response};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::task::LocalPoolHandle;
use tracing::{debug, error, info};

use crate::runtime::guard::IsolateGuard;
use crate::runtime::Runtime;
use crate::stream::{Origin, Request};

pub mod config;
mod http;
mod runtime;
pub mod stream;
pub mod params {
    use std::time::Duration;

    pub const HEAP_INIT: usize = 1 << 10;
    pub const ALIGNED_SNAPSHOT_SIZE: usize = crate::runtime::SNAPSHOT.len().next_power_of_two();
    pub const HEAP_LIMIT: usize = (128 << 20) + ALIGNED_SNAPSHOT_SIZE;
    pub const REQ_TIMEOUT: Duration = Duration::from_secs(15);
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();

    info!("Initialized POC JS service!");

    let mut listener = fn_sdk::ipc::conn_bind().await;

    // Explicitly initialize the v8 platform on the main thread
    JsRuntime::init_platform(None, false);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<IsolateHandle>();
    tokio::task::spawn(async move {
        let mut isolates = FuturesUnordered::new();
        loop {
            tokio::select! {
                next = rx.recv() => {
                    match next {
                        Some(isolate) => {
                            isolates.push(async move {
                                tokio::time::sleep(params::REQ_TIMEOUT).await;
                                isolate.terminate_execution();
                            })
                        }
                        None => break,
                    }
                }
                Some(_) = isolates.next() => {}
            }
        }
    });

    let pool = LocalPoolHandle::new(num_cpus::get());
    while let Ok(conn) = listener.accept().await {
        let tx_clone = tx.clone();
        pool.spawn_pinned(|| {
            tokio::task::spawn_local(async move {
                if let Err(e) = handle_connection(conn, tx_clone).await {
                    error!("session failed: {e:?}");
                }
            })
        });
    }
}

async fn handle_connection(
    mut connection: Connection,
    tx: UnboundedSender<IsolateHandle>,
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

            if let Err(e) = handle_request(0, &mut connection, tx, request).await {
                respond_with_error(&mut connection, format!("{e:?}").as_bytes(), 400).await?;
                return Err(e);
            }
        },
        TransportDetail::Task { depth, payload } => {
            let request: Request = serde_json::from_slice(payload)?;
            if let Err(e) = handle_request(*depth, &mut connection, tx, request).await {
                respond_with_error(&mut connection, e.to_string().as_bytes(), 400).await?;
                return Err(e);
            }
        },
        TransportDetail::Other => {
            while let Some(payload) = connection.read_payload().await {
                let request: Request = serde_json::from_slice(&payload)?;
                if let Err(e) = handle_request(0, &mut connection, tx.clone(), request).await {
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
    tx: UnboundedSender<IsolateHandle>,
    request: Request,
) -> anyhow::Result<()> {
    let Request {
        origin,
        uri,
        path,
        param,
        otel,
    } = request;
    if uri.is_empty() {
        bail!("Empty origin uri");
    }

    let module_url = match origin {
        Origin::Blake3 => format!("blake3://{uri}"),
        Origin::Ipfs => format!("ipfs://{uri}"),
        Origin::Http => uri.clone(),
        Origin::Unknown => todo!(),
    }
    .parse::<ModuleSpecifier>()
    .context("Invalid origin URI")?;

    // build the runtime config
    let mut config = FleekConfig { otel };

    // check directory for config file; if it's found, we override
    // the request configuration with its values
    if origin == Origin::Ipfs {
        let Ok(cid) = module_url.host_str().unwrap().parse::<Cid>() else {
            bail!("invalid ipfs cid");
        };
        let Some(hash) =
            fn_sdk::api::fetch_from_origin(fn_sdk::api::Origin::IPFS, cid.to_bytes()).await
        else {
            bail!("failed to fetch from origin")
        };
        let bucket = fn_sdk::blockstore::blockstore_root().await;
        let entry = bucket.get(&hash).await?;
        if entry.is_dir() {
            // check for a config file
            let mut dir = entry.into_dir().unwrap();
            if let Ok(Some(config_file)) = dir.get_entry(b"fleek.config.json").await {
                match config_file.link {
                    BorrowedLink::Content(hash) => {
                        // load file and read contents
                        let file_entry = bucket.get(hash).await?;
                        if file_entry.is_file() {
                            // load handle
                            let mut handle =
                                ContentHandle::from_file(bucket, file_entry.into_file().unwrap())
                                    .await?;
                            // read content
                            let content = handle.read_to_end().await?;

                            // parse into FleekConfig and use it
                            config = serde_json::from_slice(&content)?;
                            debug!("Config file successfully loaded: {config:?}");
                        }
                    },
                    BorrowedLink::Path(_) => bail!("symlinks are not supported for config files"),
                }
            }
        }
    }

    let mut location = module_url.clone();
    if let Some(path) = path {
        location = location.join(&path).context("Invalid path string")?;
    }

    // Create runtime and execute the source.
    let mut runtime = Runtime::new(
        location.clone(),
        depth,
        config.otel.endpoint.as_ref().map(|u| u.to_string()),
        config.otel.headers,
        config.otel.tags,
    )
    .context("Failed to initialize runtime")?;
    tx.send(runtime.deno.v8_isolate().thread_safe_handle())?;

    unsafe {
        runtime.deno.v8_isolate().exit();
    }

    let mut guard = IsolateGuard::new(runtime);
    let res = guard
        .guard(|rt| {
            Box::pin(handle_request_and_respond(
                rt, connection, module_url, param,
            ))
        })
        .await;

    let mut runtime = guard.destroy();

    unsafe {
        runtime.deno.v8_isolate().enter();
    }

    res?;

    let feed = runtime.end();
    debug!("{feed:?}");

    Ok(())
}

async fn handle_request_and_respond(
    runtime: &mut Runtime,
    connection: &mut Connection,
    module_url: ModuleSpecifier,
    param: Option<serde_json::Value>,
) -> anyhow::Result<()> {
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

    parse_and_respond(connection, runtime, res).await?;

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
        // Attempt to parse and use the value as an http response override object
        if connection.is_http_request() {
            if let Ok(http_response) = http::response::parse(scope, local) {
                respond_with_http_response(connection, http_response).await?;
                return Ok(());
            }
        }

        // Parse the response into a generic json value
        let value = serde_v8::from_v8::<serde_json::Value>(scope, local)
            .context("failed to deserialize response")?
            .clone();

        // Otherwise, send the data as a json string
        let res = serde_json::to_string(&value).context("failed to encode json response")?;
        respond(connection, res.as_bytes()).await?;
    }

    Ok(())
}
