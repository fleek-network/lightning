//! This module provides a reader abstraction to read stream data downloaded from IPFS and stored
//! in a buffer. After that it is applied to a `data_codec.rs` to decode the data into a specific
//! `fs.rs` format
use bytes::{Bytes, BytesMut};
use tokio_stream::{Stream, StreamExt as _};
use typed_builder::TypedBuilder;

use super::data_codec::{Decoder, DefaultDecoder};
use super::fs::{DocId, IpldItem};
use crate::errors::IpldError;

/// The default buffer size for the reader is 16KB.
const DEFAULT_BUFFER_SIZE: usize = 1024 * 16;

/// A reader abstraction to read stream data downloaded from IPFS and stored in a buffer.
///
/// After that it is applied to a `data_codec.rs` to decode the data into a specific `fs.rs` format.
///
/// The `IpldReader` struct is a builder pattern that allows to inject any decoder we want to use.
#[derive(Debug, TypedBuilder)]
pub struct IpldReader<D> {
    #[builder(setter(prefix = "with_", transform = |x: usize| BytesMut::with_capacity(x)), default)]
    buffer: BytesMut,
    #[builder(default, setter(strip_option))]
    last_id: Option<DocId>,
    decoder: D,
}

/// Implement the `Clone` trait for the `IpldReader` struct not not reference to the same buffer.
impl<D: Clone> Clone for IpldReader<D> {
    fn clone(&self) -> Self {
        Self {
            buffer: BytesMut::with_capacity(self.buffer.capacity()),
            last_id: self.last_id.clone(),
            decoder: self.decoder.clone(),
        }
    }
}

impl Default for IpldReader<DefaultDecoder> {
    fn default() -> Self {
        IpldReader::builder()
            .with_buffer(DEFAULT_BUFFER_SIZE)
            .decoder(DefaultDecoder::default())
            .build()
    }
}

/// Implement the `IpldReader` struct methods.
///
/// -[`D`] is the decoder type which is constraint to the `Decoder` trait.
impl<D> IpldReader<D>
where
    D: Decoder + Clone + Send + Sync + 'static,
{
    /// Read the stream data and decode it into a specific IPLD format.
    ///
    /// - [`S`] is the stream data. It is a `Stream` of `Result<Bytes, E>`.
    /// - [`I`] is the document identifier. It is something that can be converted into a `DocId`.
    /// - [`E`] is the error type for the stream data.
    pub async fn read<
        E: Into<IpldError>,
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        I: Into<DocId>,
    >(
        &mut self,
        id: I,
        stream: S,
    ) -> Result<IpldItem, IpldError> {
        let mut stream = stream;
        let doc_id = id.into();
        if doc_id.cid().codec() != 0x70 {
            return Err(IpldError::UnsupportedCodec(*doc_id.cid()));
        }
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Into::into)?;
            self.buffer.extend_from_slice(&chunk);
        }
        let item = self.decoder.decode_from_slice(&doc_id, &self.buffer)?;
        self.buffer.clear();
        self.last_id = Some(doc_id);
        Ok(item)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::LazyLock;

    use ipld_core::cid::Cid;
    #[allow(unused_imports)]
    use tokio_stream::StreamExt as _;

    use super::*;

    static FIXTURES: LazyLock<HashMap<Cid, Vec<u8>>> = LazyLock::new(load_fixtures);
    const NOT_UNIXFS: [&str; 3] = [
        "bafybeibh647pmxyksmdm24uad6b5f7tx4dhvilzbg2fiqgzll4yek7g7y4",
        "bafybeihyivpglm6o6wrafbe36fp5l67abmewk7i2eob5wacdbhz7as5obe",
        "bafybeie7xh3zqqmeedkotykfsnj2pi4sacvvsjq6zddvcff4pq7dvyenhu",
    ];

    fn load_fixtures() -> HashMap<Cid, Vec<u8>> {
        fs::read_dir("tests/fixtures")
            .unwrap()
            .filter_map(|file| {
                // Filter out invalid files.
                let file = file.ok()?;

                let path = file.path();
                let cid = path
                    .file_stem()
                    .expect("Filename must have a name")
                    .to_os_string()
                    .into_string()
                    .expect("Filename must be valid UTF-8");
                let bytes = fs::read(&path).expect("File must be able to be read");

                Some((
                    Cid::try_from(cid.clone()).expect("Filename must be a valid Cid"),
                    bytes,
                ))
            })
            .collect()
    }

    #[tokio::test]
    async fn test_ipld_fixtures_no_unixfs() {
        for cid_not_unix in NOT_UNIXFS.iter() {
            let doc_id = Cid::try_from(*cid_not_unix).unwrap();
            let item = from_cid(doc_id).await;
            assert!(item.is_err());
        }
    }

    #[tokio::test]
    async fn test_ipld_stream_fixtures() {
        for doc_id in FIXTURES.keys() {
            if NOT_UNIXFS.contains(&doc_id.to_string().as_str()) {
                continue;
            }
            let item = from_cid(*doc_id).await.unwrap();
            assert_eq!(<IpldItem as Into<Cid>>::into(item), *doc_id);
        }
    }

    async fn from_cid(doc_id: Cid) -> Result<IpldItem, IpldError> {
        let data: Result<Bytes, IpldError> =
            Ok(Bytes::copy_from_slice(FIXTURES.get(&doc_id).unwrap()));
        let stream = tokio_stream::once(data);
        let mut ipld_reader = IpldReader::default();
        ipld_reader.read(doc_id, stream).await
    }
}
