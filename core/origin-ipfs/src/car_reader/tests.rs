use cid::Cid;
use futures::TryStreamExt;
use hyper::body::{Body, Bytes};
//use libipld::pb::PbNode;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use super::reader::{header_v1, header_v2, pragma_v2, CarV1Header, CarV2Header, CarV2Pragma};
//use crate::ipld_utils::unixfs::Data;
use super::{hyper_error, utils::Buffer};
use crate::car_reader::CarReader;

#[tokio::test]
async fn test_extract_header_v1() {
    let bytes = std::fs::read("../test-utils/files/sample-v1.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let mut reader = StreamReader::new(body.map_err(hyper_error));
    let (header, _) = header_v1(&mut reader).await.unwrap();

    let cid =
        Cid::try_from("bafy2bzaced4ueelaegfs5fqu4tzsh6ywbbpfk3cxppupmxfdhbpbhzawfw5oy").unwrap();
    let target_header = CarV1Header {
        version: 1,
        roots: vec![cid],
    };
    assert_eq!(header, target_header);
}

#[tokio::test]
async fn test_extract_pragma_v2_after_failed_parsing_header_v1() {
    let bytes = std::fs::read("../test-utils/files/sample-unixfs-v2.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let mut reader = StreamReader::new(body.map_err(hyper_error));
    if let Err(e) = header_v1(&mut reader).await {
        if let Some(Buffer { data, .. }) = e.downcast_ref() {
            let pragma = pragma_v2(data).await.unwrap();
            let target_pragma = CarV2Pragma { version: 2 };

            assert_eq!(pragma, target_pragma);
        } else {
            panic!("This is a car v2 file");
        }
    } else {
        panic!("This is a car v2 file");
    }
}

#[tokio::test]
async fn test_extract_pragma_v2() {
    let bytes = std::fs::read("../test-utils/files/sample-unixfs-v2.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let mut reader = StreamReader::new(body.map_err(hyper_error));
    let mut buf = vec![0; 11];
    reader.read_exact(&mut buf).await.unwrap();
    let pragma = pragma_v2(&buf[1..]).await.unwrap();
    let target_pragma = CarV2Pragma { version: 2 };

    assert_eq!(pragma, target_pragma);
}

#[tokio::test]
async fn test_extract_header_v2() {
    let bytes = std::fs::read("../test-utils/files/sample-unixfs-v2.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let mut reader = StreamReader::new(body.map_err(hyper_error));
    let mut buf = vec![0; 11];
    reader.read_exact(&mut buf).await.unwrap();
    let pragma = pragma_v2(&buf[1..]).await.unwrap();
    let target_pragma = CarV2Pragma { version: 2 };
    assert_eq!(pragma, target_pragma);

    let header = header_v2(&mut reader).await.unwrap();
    let target_header = CarV2Header {
        characteristics: 0,
        data_offset: 51,
        data_size: 284,
        index_offset: 335,
    };
    assert_eq!(header, target_header);
}

#[tokio::test]
async fn test_extract_header_v2_wrapped() {
    let bytes = std::fs::read("../test-utils/files/sample-wrapped-v2.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let mut reader = StreamReader::new(body.map_err(hyper_error));
    let mut buf = vec![0; 11];
    reader.read_exact(&mut buf).await.unwrap();
    let pragma = pragma_v2(&buf[1..]).await.unwrap();
    let target_pragma = CarV2Pragma { version: 2 };
    assert_eq!(pragma, target_pragma);

    let header = header_v2(&mut reader).await.unwrap();
    let target_header = CarV2Header {
        characteristics: 0,
        data_offset: 51,
        data_size: 479907,
        index_offset: 479958,
    };
    assert_eq!(header, target_header);
}

#[tokio::test]
async fn test_extract_car_v2() {
    let target_bytes = std::fs::read("../test-utils/files/bird.jpg").unwrap();
    let bytes = std::fs::read("../test-utils/files/bird_v2.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let reader = StreamReader::new(body.map_err(hyper_error));
    let mut reader = CarReader::new(reader).await.unwrap();

    let mut extracted_bytes: Vec<u8> = Vec::new();
    loop {
        match reader.next_block().await {
            Ok(Some((_cid, data))) => {
                extracted_bytes.extend(&data);
            },
            Ok(None) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    assert_eq!(target_bytes, extracted_bytes);
}

#[tokio::test]
async fn test_extract_v2_wrapped() {
    let bytes = std::fs::read("../test-utils/files/sample-wrapped-v2.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let reader = StreamReader::new(body.map_err(hyper_error));

    let mut reader = CarReader::new(reader).await.unwrap();
    let mut extracted_bytes: Vec<u8> = Vec::new();
    loop {
        match reader.next_block().await {
            Ok(Some((_cid, data))) => {
                extracted_bytes.extend(&data);
            },
            Ok(None) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
}

#[tokio::test]
async fn test_extract_car_v1() {
    let target_bytes = std::fs::read("../test-utils/files/bird.jpg").unwrap();
    let bytes = std::fs::read("../test-utils/files/bird_v1.car").unwrap();
    let bytes = Bytes::from_iter(bytes);
    let body = Body::from(bytes);

    let reader = StreamReader::new(body.map_err(hyper_error));
    let mut reader = CarReader::new(reader).await.unwrap();

    let mut extracted_bytes: Vec<u8> = Vec::new();
    loop {
        match reader.next_block().await {
            Ok(Some((_cid, data))) => {
                extracted_bytes.extend(&data);
            },
            Ok(None) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    assert_eq!(target_bytes, extracted_bytes);
}
