use anyhow::{anyhow, Context};
use arrayref::array_ref;
use cid::Cid;
use deno_core::{serde_v8, v8, JsRuntime};
use tracing::{debug, error, info};

use crate::runtime::Runtime;
use crate::stream::{Origin, Request, ServiceStream};

mod runtime;
pub mod stream;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    fn_sdk::ipc::init_from_env();

    info!("Initialized POC JS service!");

    let listener = fn_sdk::ipc::conn_bind().await;

    // Explicitly initialize the v8 platform on the main thread
    JsRuntime::init_platform(None);

    while let Ok(conn) = listener.accept().await {
        let stream = ServiceStream::new(conn);

        // spawn a new thread and tokio runtime to handle the connection
        // TODO: This is very hacky and not very scalable
        // Research using deno's JsRealms to provide the script sandboxing in a single or a
        // few shared multithreaded runtimes, or use a custom work scheduler.
        std::thread::spawn(move || {
            if let Err(e) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create connection async runtime")
                .block_on(connection_loop(stream))
            {
                error!("session failed: {e:?}");
            }
        });
    }
}

async fn connection_loop(mut stream: ServiceStream) -> anyhow::Result<()> {
    while let Some(Request { origin, uri, param }) = stream.read_request().await {
        // Fetch content from origin
        let hash = match origin {
            Origin::Blake3 => {
                let hash = hex::decode(uri).context("failed to decode blake3 hash")?;
                if hash.len() != 32 {
                    return Err(anyhow!("invalid blake3 hash length"));
                }
                let hash = *array_ref![hash, 0, 32];

                if fn_sdk::api::fetch_blake3(hash).await {
                    hash
                } else {
                    return Err(anyhow!("failed to fetch file"));
                }
            },
            Origin::Ipfs => fn_sdk::api::fetch_from_origin(
                origin.into(),
                Cid::try_from(uri).context("invalid ipfs cid")?.to_bytes(),
            )
            .await
            .context("failed to fetch from origin")?,
            o => return Err(anyhow!("unknown origin: {o:?}")),
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
            Some(param) => source.push_str(&format!("main({param})")),
            None => source.push_str("main()"),
        }

        // Create runtime and execute the source
        let mut runtime = Runtime::new(hash);
        let res = runtime.exec(source).context("failed to run javascript")?;

        // Resolve async if applicable
        // TODO: figure out why `deno.resolve` doesn't drive async functions
        #[allow(deprecated)]
        let res = match runtime.deno.resolve_value(res).await {
            Ok(res) => res,
            Err(e) => {
                stream
                    .send_payload(e.to_string().as_bytes())
                    .await
                    .context("failed to send error message")?;
                return Err(e).context("failed to resolve output");
            },
        };

        {
            // Handle the return data
            let scope = &mut runtime.deno.handle_scope();
            let local = v8::Local::new(scope, res);

            if local.is_uint8_array() {
                // If the return type is a u8 array, send the raw data directly to the client
                let bytes = serde_v8::from_v8::<Vec<u8>>(scope, local)
                    .context("failed to deserialize response")?;
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
