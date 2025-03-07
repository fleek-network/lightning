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
        "http" => Origin::Http,
        _ => Origin::Unknown,
    };
    let mut uri = segments.next()?.to_string();

    if origin == Origin::Http {
        uri = urlencoding::decode(&uri).ok()?.to_string();
    }

    let mut path = String::new();
    for s in segments {
        path.push('/');
        path.push_str(s);
    }
    if path.is_empty() {
        path.push('/');
    }

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

    let query = url.query_pairs().fold(
        HashMap::<String, serde_json::Value>::new(),
        |mut map, (k, v)| {
            map.entry(k.to_string())
                .and_modify(|current| {
                    if let serde_json::Value::Array(arr) = current {
                        // Append to existing array parameter
                        arr.push(v.to_string().into());
                    } else {
                        // Upgrade parameter to an array containing previous and new values
                        let prev = std::mem::take(current);
                        *current = serde_json::Value::Array(vec![prev, v.to_string().into()]);
                    }
                })
                .or_insert(v.to_string().into());
            map
        },
    );

    let mut otel_endpoint = None;
    if let Some(v) = headers.get("otel_endpoint") {
        otel_endpoint = Some(v.parse::<Url>().ok()?);
    }
    let otel_headers = headers
        .iter()
        .filter_map(|(k, v)| {
            if k.starts_with("otel_header_") {
                // strip `otel_header_` prefix from the request header
                Some((k.replacen("otel_header_", "", 1), v.clone()))
            } else {
                None
            }
        })
        .collect();
    let otel_tags = headers
        .iter()
        .filter_map(|(k, v)| {
            if k.starts_with("otel_tag_") {
                // strip `otel_header_` prefix from the request header
                Some((k.replacen("otel_tag_", "", 1), v.clone()))
            } else {
                None
            }
        })
        .collect();

    let query = (!query.is_empty()).then_some(query);
    let headers = (!headers.is_empty()).then_some(headers);

    let param = Some(json!({
            "method": method,
            "headers": headers,
            "path": path,
            "query": query,
            "body": body,
    }));
    Some(Request {
        origin,
        uri,
        path: Some(path),
        param,
        otel_endpoint,
        otel_headers,
        otel_tags,
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
                HttpMethod::GET,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "GET",
                    "headers": null,
                    "path": "/",
                    "query": null,
                    "body": null,
                })),
                otel_endpoint: None,
                otel_headers: HashMap::new(),
                otel_tags: HashMap::new(),
            })
        );

        // Request with open telemetry headers
        let mut headers = HashMap::new();
        headers.insert("otel_endpoint".to_string(), "https://foo.bar".to_string());
        headers.insert("otel_header_test".to_string(), "foobar".to_string());
        assert_eq!(
            extract(
                &Url::parse("http://fleek/blake3/content-hash/").unwrap(),
                &headers,
                HttpMethod::GET,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "GET",
                    "headers": headers,
                    "path": "/",
                    "query": null,
                    "body": null,
                })),
                otel_endpoint: Some("https://foo.bar".parse().unwrap()),
                otel_headers: [("test".to_string(), "foobar".to_string())]
                    .into_iter()
                    .collect(),
                otel_tags: HashMap::new(),
            })
        );

        // Request with string body
        assert_eq!(
            extract(
                &Url::parse("http://fleek/blake3/content-hash/").unwrap(),
                &HashMap::new(),
                HttpMethod::GET,
                "foobar".into(),
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "GET",
                    "headers": null,
                    "path": "/",
                    "query": null,
                    "body": "foobar",
                })),
                otel_endpoint: None,
                otel_headers: HashMap::new(),
                otel_tags: HashMap::new(),
            })
        );

        // Request with json object body
        assert_eq!(
            extract(
                &Url::parse("http://fleek/blake3/content-hash/").unwrap(),
                &HashMap::new(),
                HttpMethod::GET,
                r#"{"foo": "bar"}"#.into(),
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/".to_string()),
                param: Some(json!({
                    "method": "GET",
                    "headers": null,
                    "path": "/",
                    "query": null,
                    "body": { "foo": "bar" },
                })),
                otel_endpoint: None,
                otel_headers: HashMap::new(),
                otel_tags: HashMap::new(),
            })
        );

        // Request with path
        assert_eq!(
            extract(
                &Url::parse("http://fleek/blake3/content-hash/a").unwrap(),
                &HashMap::new(),
                HttpMethod::GET,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a".to_string()),
                param: Some(json!({
                    "method": "GET",
                    "headers": null,
                    "path": "/a",
                    "query": null,
                    "body": null,
                })),
                otel_endpoint: None,
                otel_headers: HashMap::new(),
                otel_tags: HashMap::new(),
            })
        );

        // Request with bigger path and post method
        assert_eq!(
            extract(
                &Url::parse("http://fleek/blake3/content-hash/a/b").unwrap(),
                &HashMap::new(),
                HttpMethod::POST,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a/b".to_string()),
                param: Some(json!({
                    "method": "POST",
                    "headers": null,
                    "path": "/a/b",
                    "query": null,
                    "body": null,
                })),
                otel_endpoint: None,
                otel_headers: HashMap::new(),
                otel_tags: HashMap::new(),
            })
        );

        // Request with path and a query parameter
        assert_eq!(
            extract(
                &Url::parse("http://fleek/blake3/content-hash/a/b?a=4").unwrap(),
                &HashMap::new(),
                HttpMethod::GET,
                vec![],
            ),
            Some(Request {
                origin: Origin::Blake3,
                uri: "content-hash".to_string(),
                path: Some("/a/b".to_string()),
                param: Some(json!({
                    "method": "GET",
                    "headers": null,
                    "path": "/a/b",
                    "query": { "a": "4" },
                    "body": null,
                })),
                otel_endpoint: None,
                otel_headers: HashMap::new(),
                otel_tags: HashMap::new(),
            })
        );
    }
}
