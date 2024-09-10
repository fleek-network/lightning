use bytes::{Bytes, BytesMut};
use tokio_stream::{Stream, StreamExt as _};
use typed_builder::TypedBuilder;

use super::data_codec::{Decoder, DefaultDecoder};
use super::fs::{DocId, IpldItem};
use crate::errors::IpldError;

const DEFAULT_BUFFER_SIZE: usize = 1024 * 16;

#[derive(Debug, Clone, TypedBuilder)]
pub struct IpldReader<D> {
    #[builder(setter(prefix = "with_", transform = |x: usize| BytesMut::with_capacity(x)), default)]
    buffer: BytesMut,
    #[builder(default, setter(strip_option))]
    last_id: Option<DocId>,
    decoder: D,
}

impl Default for IpldReader<DefaultDecoder> {
    fn default() -> Self {
        IpldReader::builder()
            .with_buffer(DEFAULT_BUFFER_SIZE)
            .decoder(DefaultDecoder::default())
            .build()
    }
}

impl<D> IpldReader<D>
where
    D: Decoder + Clone + Send + Sync + 'static,
{
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
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Into::into)?;
            self.buffer.extend_from_slice(&chunk);
        }
        let doc_id = id.into();
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
