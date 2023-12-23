//! TODO: fix crash when running 2 consecutive requests

use arrayref::array_ref;
use deno_core::{serde_v8, v8};
use runtime::Runtime;
use serde::{Deserialize, Serialize};
use stream::Origin;
use tracing::info;

use crate::stream::ServiceStream;

pub(crate) mod extensions;
pub(crate) mod runtime;
pub mod stream;

// TODO: Use this
#[derive(Serialize, Deserialize)]
pub struct Request {
    origin: u8,
    uri: Vec<u8>,
    entrypoint: serde_json::Value,
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    fn_sdk::ipc::init_from_env();

    info!("Initialized POC JS service!");

    // TODO: create a snapshot runtime with the extensions loaded and initialized

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        let stream = ServiceStream::new(conn);

        // spawn a new thread and tokio runtime to handle the connection
        // TODO: This is very hacky and not very scalable
        // Research using deno's JsRealms to provide the script sandboxing in a single or a
        // few shared multithreaded runtimes.
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create connection async runtime")
                .block_on(connection_loop(stream))
        });
    }
}

async fn connection_loop(mut stream: ServiceStream) {
    while let Some((origin, uri)) = stream.read_request().await {
        let hash = match origin {
            Origin::Blake3 => {
                let hash = *array_ref![uri, 0, 32];
                if fn_sdk::api::fetch_blake3(hash).await {
                    hash
                } else {
                    panic!("failed to fetch file");
                }
            },
            Origin::Ipfs => fn_sdk::api::fetch_from_origin(origin.into(), uri)
                .await
                .expect("failed to fetch from origin"),
            o => unimplemented!("unknown origin: {o:?}"),
        };

        let source_bytes = fn_sdk::blockstore::ContentHandle::load(&hash)
            .await
            .expect("failed to get handle for source from blockstore")
            .read_to_end()
            .await
            .expect("failed to read source from blockstore");
        let source = String::from_utf8(source_bytes).expect("failed to parse source as utf8");

        let response = {
            let mut runtime = Runtime::new();
            let res = runtime.exec(source).expect("failed to run javascript");

            let res = runtime
                .deno
                .resolve_value(res)
                .await
                .expect("failed to resolve output");

            // deserialize the response to json
            let scope = &mut runtime.deno.handle_scope();
            let local = v8::Local::new(scope, res);
            let value = serde_v8::from_v8::<serde_json::Value>(scope, local)
                .expect("failed to deserialize response")
                .clone();

            serde_json::to_string(&value).unwrap()
        };

        stream
            .send_payload(response.as_bytes())
            .await
            .expect("failed to send response");
    }
}
