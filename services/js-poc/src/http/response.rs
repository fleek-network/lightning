use anyhow::{anyhow, Context, Result};
use fn_sdk::header::HttpResponse;

enum HeaderFormat {
    Undetermined,
    SingleValue,
    ValueArray,
    Object,
}

pub fn parse(value: &serde_json::Value) -> Result<HttpResponse> {
    let body = parse_body(value)?;
    let status = parse_status(value)?;
    let headers = parse_headers(value)?;

    Ok(HttpResponse {
        headers: Some(headers),
        status: Some(status),
        body,
    })
}

fn parse_headers(value: &serde_json::Value) -> Result<Vec<(String, Vec<String>)>> {
    if value["headers"].is_null() {
        return Err(anyhow!("Headers are missing"));
    }
    if let Some(headers_j) = value["headers"].as_array() {
        // the headers are wrapped into an array
        let mut headers = Vec::new();
        for header_j in headers_j {
            let (name, value) = parse_header(header_j)?;
            headers.push((name, vec![value]));
        }
        return Ok(headers);
    } else if let Some(headers_j) = value["headers"].as_object() {
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

                let value = value
                    .as_str()
                    .context("Failed to convert value to string")?;
                headers.push((key.to_owned(), vec![value.to_string()]));
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
                        let elem = elem.as_str().context("Failed to convert value to string")?;
                        header_values.push(elem.to_string());
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
                        let (header_name, header_val) = parse_header(elem)?;
                        if *key != header_name.to_lowercase() {
                            return Err(anyhow!("Inconsistent header names"));
                        }
                        header_values.push(header_val);
                    }
                }
                headers.push((key.to_owned(), header_values));
            } else {
                let (name, value) = parse_header(value)?;
                headers.push((name, vec![value]));
            }
        }
        return Ok(headers);
    }

    Err(anyhow!("Unsupported header format"))
}

fn parse_header(value: &serde_json::Value) -> Result<(String, String)> {
    if value.is_object() && !value["key"].is_null() && !value["value"].is_null() {
        if let Ok((name, value)) = parse_header_key_value(value) {
            return Ok((name, value));
        }
    }
    Err(anyhow!("Unsupported header format"))
}

fn parse_header_key_value(value: &serde_json::Value) -> Result<(String, String)> {
    if value["key"].is_null() || value["value"].is_null() {
        return Err(anyhow!("Key or value is missing"));
    }
    let key = value["key"]
        .as_str()
        .context("Failed to convert key to string")?;
    let value = value["value"]
        .as_str()
        .context("Failed to convert value to string")?;
    Ok((key.to_string(), value.to_string()))
}

fn parse_body(value: &serde_json::Value) -> Result<String> {
    if value["body"].is_null() {
        return Err(anyhow!("Body is missing"));
    }
    // TODO(matthias): support other types than string?
    let body = value["body"]
        .as_str()
        .context("Failed to convert body to string")?;
    Ok(body.to_string())
}

fn parse_status(value: &serde_json::Value) -> Result<u16> {
    if value["status"].is_null() {
        return Err(anyhow!("Status is missing"));
    }

    let status = if let Some(status) = value["status"].as_u64() {
        status as u16
    } else {
        let status = value["status"]
            .as_str()
            .context("Failed to convert status to string")?;
        status.parse::<u16>()?
    };
    Ok(status)
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
