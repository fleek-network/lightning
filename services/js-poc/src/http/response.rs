use anyhow::{anyhow, Context, Result};
use fn_sdk::header::HttpResponse;

enum HeaderFormat {
    Undetermined,
    SingleValue,
    ValueArray,
    Object,
}

pub fn parse(value: &serde_json::Value) -> Result<HttpResponse> {
    let body = value_to_string(value.get("body").context("Body is missing")?);
    let status = parse_status(value)?;
    let headers = parse_headers(value)?;

    Ok(HttpResponse {
        headers: Some(headers),
        status: Some(status),
        body,
    })
}

fn parse_headers(value: &serde_json::Value) -> Result<Vec<(String, Vec<String>)>> {
    let headers = value.get("headers").context("Missing headers")?;
    if headers.is_null() {
        return Err(anyhow!("Headers cannot be null"));
    }

    if let Some(headers_j) = headers.as_array() {
        // the headers are wrapped into an array
        let mut headers = Vec::new();
        for header_j in headers_j {
            let (name, values) = parse_header(header_j)?;
            headers.push((name, values));
        }
        return Ok(headers);
    } else if let Some(headers_j) = headers.as_object() {
        let mut headers = Vec::new();

        let mut header_fmt = HeaderFormat::Undetermined;
        for (key, value) in headers_j {
            if value.is_string() {
                // For each header name `key` in the headers object, there is a single header
                // value stored in `value`.
                match header_fmt {
                    HeaderFormat::Undetermined => {
                        header_fmt = HeaderFormat::SingleValue;
                    },
                    HeaderFormat::SingleValue => (),
                    _ => {
                        return Err(anyhow!("Mixing different header formats is not supported"));
                    },
                }

                headers.push((
                    key.to_owned(),
                    vec![
                        value
                            .as_str()
                            .context("Failed to convert value to string")?
                            .to_string(),
                    ],
                ));
            } else if let Some(array) = value.as_array() {
                // At this point the array either consists of header values corresponding to
                // the header name stored in `key` or key value objects, where key and value
                // are header name and header value, respectively.

                let mut header_values = Vec::new();
                for elem in array {
                    if elem.is_string() {
                        match header_fmt {
                            HeaderFormat::Undetermined => {
                                header_fmt = HeaderFormat::ValueArray;
                            },
                            HeaderFormat::ValueArray => (),
                            _ => {
                                return Err(anyhow!(
                                    "Mixing different header formats is not supported"
                                ));
                            },
                        }
                        header_values.push(
                            elem.as_str()
                                .context("Failed to convert value to string")?
                                .to_string(),
                        );
                    } else {
                        // TODO(matthias): make sure all of the objects share the same key
                        match header_fmt {
                            HeaderFormat::Undetermined => {
                                header_fmt = HeaderFormat::Object;
                            },
                            HeaderFormat::Object => (),
                            _ => {
                                return Err(anyhow!(
                                    "Mixing different header formats is not supported"
                                ));
                            },
                        }
                        let (header_name, ref mut values) = parse_header(elem)?;
                        if *key != header_name.to_lowercase() {
                            return Err(anyhow!("Inconsistent header names"));
                        }
                        header_values.append(values);
                    }
                }
                headers.push((key.to_owned(), header_values));
            } else {
                let (name, values) = parse_header(value)?;
                headers.push((name, values));
            }
        }
        return Ok(headers);
    }

    Err(anyhow!("Unsupported header format"))
}

fn parse_header(value: &serde_json::Value) -> Result<(String, Vec<String>)> {
    if value.is_object() && !value["key"].is_null() && !value["value"].is_null() {
        Ok((
            value["key"]
                .as_str()
                .context("Header key must be a string")?
                .to_string(),
            vec![value_to_string(&value["value"])],
        ))
    } else if let Some(arr) = value.as_array() {
        match arr.as_slice() {
            // TODO: verify correctness of allowing empty headers
            [serde_json::Value::String(key)] => Ok((key.clone(), vec![])),
            [
                serde_json::Value::String(key),
                serde_json::Value::Array(arr),
            ] => Ok((key.clone(), arr.iter().map(value_to_string).collect())),
            [serde_json::Value::String(key), _, ..] => {
                Ok((key.clone(), arr[1..].iter().map(value_to_string).collect()))
            },
            [_, ..] => Err(anyhow!("Header key must be a string")),
            [] => Err(anyhow!("Empty header key value array pair")),
        }
    } else {
        Err(anyhow!("Unsupported header format"))
    }
}

fn parse_status(value: &serde_json::Value) -> Result<u16> {
    let status = value.get("status").context("Status is missing")?;
    if status.is_null() {
        return Err(anyhow!("Status cannot be null"));
    }

    if let Some(status) = status.as_u64() {
        u16::try_from(status).context("Invalid status code")
    } else {
        let status = value["status"]
            .as_str()
            .context("Invalid status code, expected string or integer")?;
        status.parse::<u16>().context("Invalid status code")
    }
}

/// Turn any value into a string, ignoring quotes if the value is a string already
fn value_to_string(value: &serde_json::Value) -> String {
    if let Some(s) = value.as_str() {
        s.into()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use fn_sdk::header::HttpResponse;

    use super::*;

    #[test]
    fn test_array_of_key_val_objects() {
        let res = r###"
{
  "headers": [
    {"key": "content-type", "value": "text"},
    {"key": "content-length", "value": "300"}
  ],
  "status": 200,
  "body": "hello"
}
        "###;

        let target = HttpResponse {
            status: Some(200),
            headers: Some(vec![
                ("content-type".to_string(), vec!["text".to_string()]),
                ("content-length".to_string(), vec!["300".to_string()]),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_array_of_key_val_objects_multiple_values() {
        let res = r###"
{
  "status":"201",
  "headers":{
    "content-type":[
      {
        "key":"content-type",
        "value":"text/html; charset=utf-8"
      }
    ],
    "vary":[
      {
        "key":"vary",
        "value":"RSC"
      },
      {
        "key":"vary",
        "value":"Next-Router-State-Tree"
      },
      {
        "key":"vary",
        "value":"Next-Router-Prefetch"
      }
    ]
  },
  "body": "hello"
}
        "###;

        let target = HttpResponse {
            status: Some(201),
            headers: Some(vec![
                (
                    "content-type".to_string(),
                    vec!["text/html; charset=utf-8".to_string()],
                ),
                (
                    "vary".to_string(),
                    vec![
                        "RSC".to_string(),
                        "Next-Router-State-Tree".to_string(),
                        "Next-Router-Prefetch".to_string(),
                    ],
                ),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_header_array_variants() {
        let res = r###"
{
  "status":"201",
  "headers": [
      ["empty-header"],
      ["simple-header", "foo"],
      ["multi-header", [ "foo", "bar" ]],
      ["encoded-number-header", 1],
      ["encoded-object-header", { "foo": "bar" }]
  ],
  "body": "hello"
}
        "###;

        let target = HttpResponse {
            status: Some(201),
            headers: Some(vec![
                ("empty-header".to_string(), vec![]),
                ("simple-header".to_string(), vec!["foo".to_string()]),
                (
                    "multi-header".to_string(),
                    vec!["foo".to_string(), "bar".to_string()],
                ),
                ("encoded-number-header".to_string(), vec!["1".to_string()]),
                (
                    "encoded-object-header".to_string(),
                    vec![r#"{"foo":"bar"}"#.to_string()],
                ),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_key_val_objects() {
        let res = r###"
{
  "status":"200",
  "headers":{
    "content-type":{
      "key":"content-type",
      "value":"text/html; charset=utf-8"
    },
    "vary":{
      "key":"vary",
      "value":"RSC"
    }
  },
  "body": "hello world"
}
        "###;

        let target = HttpResponse {
            status: Some(200),
            headers: Some(vec![
                (
                    "content-type".to_string(),
                    vec!["text/html; charset=utf-8".to_string()],
                ),
                ("vary".to_string(), vec!["RSC".to_string()]),
            ]),
            body: "hello world".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_key_val_objects_dummy_value() {
        let res = r###"
{
  "status":"200",
  "headers":{
    "content-type":{
      "key":"content-type",
      "value":"text/html; charset=utf-8"
    },
    "vary":{
      "key":"vary",
      "value":"RSC"
    }
  },
  "body": "hello",
  "dummy": "abc"
}
        "###;

        let target = HttpResponse {
            status: Some(200),
            headers: Some(vec![
                (
                    "content-type".to_string(),
                    vec!["text/html; charset=utf-8".to_string()],
                ),
                ("vary".to_string(), vec!["RSC".to_string()]),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_key_val_object_list_for_each_header_name() {
        let res = r###"
{
  "headers": {
    "content-type": [
    {
      "key": "Content-Type",
      "value": "text/html; charset=utf-8"
    }
    ],
    "vary": [
    {
      "key": "Vary",
      "value": "RSC"
    },
    {
      "key": "Vary",
      "value": "Next-Router-State-Tree"
    }
    ]
  },
  "status": "200",
  "body": "hello"
}
        "###;

        let target = HttpResponse {
            status: Some(200),
            headers: Some(vec![
                (
                    "content-type".to_string(),
                    vec!["text/html; charset=utf-8".to_string()],
                ),
                (
                    "vary".to_string(),
                    vec!["RSC".to_string(), "Next-Router-State-Tree".to_string()],
                ),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_key_val_object() {
        let res = r###"
{
  "status":"200",
  "headers":{
    "content-type":"text/html; charset=utf-8",
    "vary":"RSC"
  },
  "body": "hello"
}
        "###;

        let target = HttpResponse {
            status: Some(200),
            headers: Some(vec![
                (
                    "content-type".to_string(),
                    vec!["text/html; charset=utf-8".to_string()],
                ),
                ("vary".to_string(), vec!["RSC".to_string()]),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_list_of_values_for_each_header_name() {
        let res = r###"
{
  "status":"200",
  "headers":{
    "content-type":["text/html; charset=utf-8"],
    "vary":["RSC", "Next-Router-State-Tree", "Next-Router-Prefetch"]
  },
  "body": "hello"
}
        "###;

        let target = HttpResponse {
            status: Some(200),
            headers: Some(vec![
                (
                    "content-type".to_string(),
                    vec!["text/html; charset=utf-8".to_string()],
                ),
                (
                    "vary".to_string(),
                    vec![
                        "RSC".to_string(),
                        "Next-Router-State-Tree".to_string(),
                        "Next-Router-Prefetch".to_string(),
                    ],
                ),
            ]),
            body: "hello".to_string(),
        };

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        let http_res = parse(&value).unwrap();

        assert_eq!(http_res, target);
    }

    #[test]
    fn test_inconsistent_header_names() {
        let res = r###"
{
  "headers": {
    "content-type": [
    {
      "key": "Content-Type",
      "value": "text/html; charset=utf-8"
    }
    ],
    "vary": [
    {
      "key": "Vary",
      "value": "RSC"
    },
    {
      "key": "Content-Length",
      "value": "1000"
    }
    ]
  },
  "status": "200",
  "body": "hello"
}
        "###;

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();

        assert!(parse(&value).is_err());
    }

    #[test]
    fn test_inconsistent_header_formats() {
        let res = r###"
{
  "headers": {
    "content-type": [
      {
        "key": "Content-Type",
        "value": "text/html; charset=utf-8"
      }
    ],
    "vary":["RSC", "Next-Router-State-Tree", "Next-Router-Prefetch"]
  },
  "status": "200",
  "body": "hello"
}
        "###;

        let value = serde_json::from_str::<serde_json::Value>(res).unwrap();
        assert!(parse(&value).is_err());
    }
}
