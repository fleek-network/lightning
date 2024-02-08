use anyhow::{anyhow, Context};
use arrayref::array_ref;
use cid::Cid;
use deno_core::v8::IsolateHandle;
use deno_core::{serde_v8, v8, JsRuntime};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};

use crate::runtime::Runtime;
use crate::stream::{Origin, Request, ServiceStream};

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

    let listener = fn_sdk::ipc::conn_bind().await;

    // Explicitly initialize the v8 platform on the main thread
    JsRuntime::init_platform(None);

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

    while let Ok(conn) = listener.accept().await {
        let stream = ServiceStream::new(conn);
        let tx = tx.clone();

        // spawn a new thread and tokio runtime to handle the connection
        // TODO: This is very hacky and not very scalable
        // Research using deno's JsRealms to provide the script sandboxing in a single or a
        // few shared multithreaded runtimes, or use a custom work scheduler.
        std::thread::spawn(move || {
            if let Err(e) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create connection async runtime")
                .block_on(connection_loop(tx, stream))
            {
                error!("session failed: {e:?}");
            }
        });

        tokio::spawn(async move {});
    }
}

async fn connection_loop(
    tx: UnboundedSender<IsolateHandle>,
    mut stream: ServiceStream,
) -> anyhow::Result<()> {
    while let Some(Request {
        origin,
        uri,
        path,
        param,
    }) = stream.read_request().await
    {
        // Fetch content from origin
        let hash = match origin {
            Origin::Blake3 => {
                let hash = hex::decode(uri).context("failed to decode blake3 hash")?;
                if hash.len() != 32 {
                    stream
                        .send_payload(b"Invalid blake3 hash length")
                        .await
                        .context("failed to send error message")?;
                    return Err(anyhow!("invalid blake3 hash length"));
                }
                let hash = *array_ref![hash, 0, 32];

                if fn_sdk::api::fetch_blake3(hash).await {
                    hash
                } else {
                    stream
                        .send_payload(b"Failed to fetch blake3 content")
                        .await
                        .context("failed to send error message")?;
                    return Err(anyhow!("failed to fetch file"));
                }
            },
            Origin::Ipfs => {
                match fn_sdk::api::fetch_from_origin(
                    origin.into(),
                    Cid::try_from(uri).context("invalid ipfs cid")?.to_bytes(),
                )
                .await
                {
                    Some(_) => todo!(),
                    None => {
                        stream
                            .send_payload(b"Failed to fetch from origin")
                            .await
                            .context("failed to send error message")?;
                        return Err(anyhow!("failed to fetch from origin"));
                    },
                }
            },
            o => {
                let err = anyhow!("unknown origin: {o:?}");
                stream
                    .send_payload(err.to_string().as_bytes())
                    .await
                    .context("failed to send error message")?;
                return Err(err);
            },
        };

        // Read and parse source from the blockstore
        let source_bytes = fn_sdk::blockstore::ContentHandle::load(&hash)
            .await
            .context("failed to get handle for source from blockstore")?
            .read_to_end()
            .await
            .context("failed to read source from blockstore")?;
        let mut source =
            String::from_utf8(source_bytes).context("failed to parse source as utf8")?;

        // Append entry point function with parameters
        match param {
            Some(param) => source.push_str(&format!("\nmain({param})")),
            None => source.push_str("\nmain()"),
        }

        // Create runtime and execute the source
        let mut runtime = match Runtime::new(hash, path) {
            Ok(runtime) => runtime,
            Err(e) => {
                stream
                    .send_payload(e.to_string().as_bytes())
                    .await
                    .context("failed to send error message")?;
                return Err(e).context("failed to initialize runtime");
            },
        };
        tx.send(runtime.deno.v8_isolate().thread_safe_handle())
            .context("Failed to send the IsolateHandle to main thread.")?;

        let res = match runtime.exec(source) {
            Ok(res) => res,
            Err(e) => {
                stream
                    .send_payload(e.to_string().as_bytes())
                    .await
                    .context("failed to send error message")?;
                return Err(e).context("failed to run javascript");
            },
        };

        // Submit the hash of the executed js
        fn_sdk::api::submit_js_hash(1, hash).await;

        // Resolve async if applicable
        // TODO: figure out why `deno.resolve` doesn't drive async functions
        #[allow(deprecated)]
        let res = match tokio::time::timeout(params::REQ_TIMEOUT, runtime.deno.resolve_value(res))
            .await
        {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => {
                stream
                    .send_payload(e.to_string().as_bytes())
                    .await
                    .context("failed to send error message")?;
                return Err(e).context("failed to resolve output");
            },
            Err(e) => {
                stream
                    .send_payload(b"Request timeout")
                    .await
                    .context("failed to send error message")?;
                return Err(e).context("execution timeout");
            },
        };

        {
            // Handle the return data
            let scope = &mut runtime.deno.handle_scope();
            let local = v8::Local::new(scope, res);

            if local.is_uint8_array() || local.is_array_buffer() {
                // If the return type is a u8 array, send the raw data directly to the client
                let bytes = match deno_core::_ops::to_v8_slice_any(local) {
                    Ok(slice) => slice.to_vec(),
                    Err(e) => return Err(anyhow!("failed to parse bytes: {e}")),
                };
                stream
                    .send_payload(&bytes)
                    .await
                    .context("failed to send byte response")?;
            } else if local.is_string() {
                // Likewise for string types
                let string = serde_v8::from_v8::<String>(scope, local)
                    .context("failed to deserialize response string")?;
                stream
                    .send_payload(string.as_bytes())
                    .await
                    .context("failed to send string response")?;
            } else {
                // Otherwise, send the data as json
                let value = serde_v8::from_v8::<serde_json::Value>(scope, local)
                    .context("failed to deserialize response")?
                    .clone();
                let res =
                    serde_json::to_string(&value).context("failed to encode json response")?;
                stream
                    .send_payload(res.as_bytes())
                    .await
                    .context("failed to send json response")?;
            }
        }

        let feed = runtime.end();
        debug!("{feed:?}");
    }

    Ok(())
}
