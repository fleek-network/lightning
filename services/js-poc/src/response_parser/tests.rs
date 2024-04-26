use fn_sdk::header::HttpResponse;

use crate::response_parser::parse;

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
