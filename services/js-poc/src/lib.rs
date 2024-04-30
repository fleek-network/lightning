use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use arrayref::array_ref;
use cid::Cid;
use deno_core::url::Url;
use deno_core::v8::{Global, IsolateHandle, Value};
use deno_core::{serde_v8, v8, JsRuntime};
use fn_sdk::connection::Connection;
use fn_sdk::header::{HttpMethod, TransportDetail};
use fn_sdk::http_util::{respond, respond_with_error, respond_with_http_response};
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};

use crate::runtime::Runtime;
use crate::stream::{Origin, Request};

mod response_parser;
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
                .block_on(handle_connection(tx, conn))
            {
                error!("session failed: {e:?}");
            }
        });
    }
}

async fn handle_connection(
    tx: UnboundedSender<IsolateHandle>,
    mut connection: Connection,
) -> anyhow::Result<()> {
    if connection.is_http_request() {
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
        let request = extract_request(url, header, method, body.to_vec())
            .context("failed to parse request")?;
        handle_request(&mut connection, &tx, request).await?;
    } else {
        while let Some(payload) = connection.read_payload().await {
            let request: Request = serde_json::from_slice(&payload)?;
            handle_request(&mut connection, &tx, request).await?;
        }
    }

    Ok(())
}

async fn handle_request(
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

    // Fetch content from origin
    let hash = match origin {
        Origin::Blake3 => {
            let hash = hex::decode(uri).context("failed to decode blake3 hash")?;

            if hash.len() != 32 {
                respond_with_error(connection, b"Invalid blake3 hash length", 400).await?;
                return Err(anyhow!("invalid blake3 hash length"));
            }

            let hash = *array_ref![hash, 0, 32];

            if fn_sdk::api::fetch_blake3(hash).await {
                hash
            } else {
                respond_with_error(connection, b"Failed to fetch blake3 content", 400).await?;
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
                Some(hash) => hash,
                None => {
                    respond_with_error(connection, b"Failed to fetch from origin", 400).await?;
                    return Err(anyhow!("failed to fetch from origin"));
                },
            }
        },
        o => {
            let err = anyhow!("unknown origin: {o:?}");
            respond_with_error(connection, err.to_string().as_bytes(), 400).await?;
            return Err(err);
        },
    };

    let mut location = Url::parse(&format!("blake3://{}", hex::encode(hash)))
        .context("failed to create base url")?;
    if let Some(path) = path {
        location = location.join(&path).context("invalid path string")?;
    }

    // Read and parse the source from the blockstore
    let source_bytes = fn_sdk::blockstore::ContentHandle::load(&hash)
        .await
        .context("failed to get handle for source from blockstore")?
        .read_to_end()
        .await
        .context("failed to read source from blockstore")?;
    let source = String::from_utf8(source_bytes).context("failed to parse source as utf8")?;

    // Create runtime and execute the source
    let mut runtime = match Runtime::new(location.clone()) {
        Ok(runtime) => runtime,
        Err(e) => {
            respond_with_error(connection, e.to_string().as_bytes(), 400).await?;
            return Err(e).context("failed to initialize runtime");
        },
    };

    tx.send(runtime.deno.v8_isolate().thread_safe_handle())
        .context("Failed to send the IsolateHandle to main thread.")?;

    let res = match runtime.exec(location, source, param).await {
        Ok(Some(res)) => res,
        Ok(None) => {
            respond_with_error(connection, b"no response available", 400).await?;
            bail!("no response available");
        },
        Err(e) => {
            respond_with_error(connection, e.to_string().as_bytes(), 400).await?;
            return Err(e).context("failed to run javascript");
        },
    };

    // Resolve async if applicable
    // TODO: figure out why `deno.resolve` doesn't drive async functions
    #[allow(deprecated)]
    let res = match tokio::time::timeout(params::REQ_TIMEOUT, runtime.deno.resolve_value(res)).await
    {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => {
            respond_with_error(connection, e.to_string().as_bytes(), 400).await?;
            return Err(e).context("failed to resolve output");
        },
        Err(e) => {
            respond_with_error(connection, b"Request timeout", 400).await?;
            return Err(e).context("execution timeout");
        },
    };

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
            Err(e) => return Err(anyhow!("failed to parse bytes: {e}")),
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
            if let Ok(http_response) = response_parser::parse(&value) {
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

fn extract_request(
    url: &Url,
    headers: &HashMap<String, String>,
    method: HttpMethod,
    body: Vec<u8>,
) -> Option<Request> {
    // url = http://fleek/:origin/:hash
    let mut segments = url.path_segments()?;
    let origin = match segments.next()? {
        "blake3" => Origin::Blake3,
        "ipfs" => Origin::Ipfs,
        _ => Origin::Unknown,
    };
    let uri = segments.next()?.to_string();

    let mut path = String::new();
    for s in segments {
        path.push('/');
        path.push_str(s);
    }
    if path.is_empty() {
        path.push('/');
    }

    let frag = url.fragment().map(|f| f.to_string());

    let body = (!body.is_empty())
        .then(|| {
            // Parse input as a json value
            if let Ok(body) = serde_json::from_slice::<serde_json::Value>(&body) {
                Some(body)
            } else {
                // Otherwise, parse input as a string
                Some(String::from_utf8(body).ok()?.into())
            }
        })
        .flatten();

    let query: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let query = (!query.is_empty()).then_some(query);
    let headers = (!headers.is_empty()).then_some(headers);

    let param = Some(json!({
            "method": method,
            "headers": headers,
            "path": path,
            "fragment": frag,
            "query": query,
            "body": body,
    }));
    Some(Request {
        origin,
        uri,
        path: Some(path),
        param,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_request() {
        // Simple request
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/").unwrap(),
                &HashMap::new(),
                HttpMethod::Get,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "Get",
                    "headers": null,
                    "path": "/",
                    "fragment": null,
                    "query": null,
                    "body": null,
                })),
            })
        );

        // Request with string body
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/").unwrap(),
                &HashMap::new(),
                HttpMethod::Get,
                "foobar".into(),
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "Get",
                    "headers": null,
                    "path": "/",
                    "fragment": null,
                    "query": null,
                    "body": "foobar",
                })),
            })
        );

        // Request with json object body
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/").unwrap(),
                &HashMap::new(),
                HttpMethod::Get,
                r#"{"foo": "bar"}"#.into(),
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "Get",
                    "headers": null,
                    "path": "/",
                    "fragment": null,
                    "query": null,
                    "body": { "foo": "bar" },
                })),
            })
        );

        // Request with path
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/a").unwrap(),
                &HashMap::new(),
                HttpMethod::Get,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a".to_string()),
                param: Some(json!({
                    "method": "Get",
                    "headers": null,
                    "path": "/a",
                    "fragment": null,
                    "query": null,
                    "body": null,
                })),
            })
        );

        // Request with bigger path and post method
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/a/b").unwrap(),
                &HashMap::new(),
                HttpMethod::Post,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a/b".to_string()),
                param: Some(json!({
                    "method": "Post",
                    "headers": null,
                    "path": "/a/b",
                    "fragment": null,
                    "query": null,
                    "body": null,
                })),
            })
        );

        // Request with path and a query parameter
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/a/b?a=4").unwrap(),
                &HashMap::new(),
                HttpMethod::Get,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a/b".to_string()),
                param: Some(json!({
                    "method": "Get",
                    "headers": null,
                    "path": "/a/b",
                    "fragment": null,
                    "query": { "a": "4" },
                    "body": null,
                })),
            })
        );

        // Request with path, a query parameter, and a url fragment
        assert_eq!(
            extract_request(
                &Url::parse("http://fleek/blake3/content-hash/a/b?a=4#hello").unwrap(),
                &HashMap::new(),
                HttpMethod::Get,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a/b".to_string()),
                param: Some(json!({
                    "method": "Get",
                    "headers": null,
                    "path": "/a/b",
                    "fragment": "hello",
                    "query": { "a": "4" },
                    "body": null,
                })),
            })
        );
    }
}
