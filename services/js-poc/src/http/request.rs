use std::collections::HashMap;

use deno_core::url::Url;
use fn_sdk::header::HttpMethod;
use serde_json::json;

use crate::stream::{Origin, Request};

pub fn extract(
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
            extract(
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
            extract(
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
            extract(
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
            extract(
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
            extract(
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
            extract(
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
            extract(
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
