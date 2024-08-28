use std::ops::Deref;
use std::pin::Pin;
use std::str::FromStr;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::TryFuture;
use ipld_core::cid::Cid;
use ipld_dagpb::PbLink;
use tokio_stream::Stream;

use self::errors::IpldError;

pub mod errors;

#[derive(Clone, Debug)]
pub struct DocId(Cid);

impl Deref for DocId {
    type Target = Cid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Cid> for DocId {
    fn from(cid: Cid) -> Self {
        Self(cid)
    }
}

impl TryFrom<&str> for DocId {
    type Error = IpldError;
    fn try_from(t: &str) -> Result<Self, Self::Error> {
        DocId::from_str(t)
    }
}

impl FromStr for DocId {
    type Err = IpldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Cid::from_str(s)?))
    }
}

#[derive(Clone)]
pub struct DocDir {
    id: DocId,
    links: Vec<PbLink>,
}

impl DocDir {
    pub fn new(id: impl Into<DocId>, links: Vec<PbLink>) -> Self {
        Self {
            id: id.into(),
            links,
        }
    }

    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn links(&self) -> &Vec<PbLink> {
        &self.links
    }
}

#[derive(Clone)]
pub struct DocFile {
    id: DocId,
    name: Option<String>,
    data: Vec<u8>,
    size: Option<u64>,
}

impl DocFile {
    pub fn new(
        id: impl Into<DocId>,
        name: Option<String>,
        data: Vec<u8>,
        size: Option<u64>,
    ) -> Self {
        Self {
            id: id.into(),
            name,
            data,
            size,
        }
    }

    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn size(&self) -> &Option<u64> {
        &self.size
    }
}

pub enum IpldItem {
    Dir(DocDir),
    File(DocFile),
}

#[async_trait]
pub trait Processor {
    type Codec;

    async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError>;
}

pub struct IpldStream<P: Processor + Unpin + Send + Sync> {
    processor: P,
    current_id: Option<DocId>,
    current_dir: Option<DocDir>,
    current_links: Vec<PbLink>,
    current_index: usize,
    pending_future: Option<BoxFuture<'static, Result<Option<IpldItem>, IpldError>>>,
}

impl<P: Processor + Unpin + Send + Sync> IpldStream<P> {
    pub fn new(processor: P, id: DocId) -> Self {
        Self {
            processor,
            current_id: Some(id),
            current_dir: None,
            current_links: Vec::new(),
            current_index: 0,
            pending_future: None,
        }
    }
}

impl<P: Processor + Unpin + Send + Sync + Clone + 'static> Stream for IpldStream<P> {
    type Item = Result<IpldItem, IpldError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(fut) = this.pending_future.as_mut() {
            match Pin::new(fut).try_poll(cx) {
                Poll::Ready(result) => {
                    this.pending_future = None;
                    match result {
                        Ok(Some(IpldItem::Dir(dir))) => {
                            this.current_dir = Some(dir.clone());
                            this.current_links = dir.links.clone();
                            this.current_index = 0;
                            return Poll::Ready(Some(Ok(IpldItem::Dir(dir))));
                        },
                        Ok(Some(IpldItem::File(file))) => {
                            return Poll::Ready(Some(Ok(IpldItem::File(file))));
                        },
                        Ok(None) | Err(_) => {
                            return Poll::Ready(None);
                        },
                    }
                },
                Poll::Pending => {
                    return Poll::Pending;
                },
            }
        }

        if let Some(id) = this.current_id.take() {
            let processor = this.processor.clone();
            this.pending_future = Some(Box::pin(async move { processor.get(id).await }));
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        if let Some(dir) = &this.current_dir {
            if this.current_index < this.current_links.len() {
                let link = &this.current_links[this.current_index];
                this.current_index += 1;
                this.current_id = Some(link.cid.into());
                return Poll::Ready(Some(Ok(IpldItem::Dir(dir.clone()))));
            } else {
                this.current_dir = None;
                this.current_links.clear();
                this.current_index = 0;
            }
        }

        Poll::Ready(None)
    }
}

mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::LazyLock;

    use ipld_core::cid::Cid;
    use ipld_core::codec::Codec;
    use ipld_dagpb::{DagPbCodec, PbNode};
    #[allow(unused_imports)]
    use tokio_stream::StreamExt as _;

    use super::*;

    static FIXTURES: LazyLock<HashMap<Cid, Vec<u8>>> = LazyLock::new(load_fixtures);

    #[derive(Clone)]
    struct MyProcessor;

    #[async_trait]
    impl Processor for MyProcessor {
        type Codec = ipld_dagpb::DagPbCodec;

        async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError> {
            let data = b"Hello, world!".to_vec();
            let size = Some(data.len() as u64);
            let file = DocFile::new(id, None, data, size);
            Ok(Some(IpldItem::File(file)))
        }
    }

    fn load_fixtures() -> HashMap<Cid, Vec<u8>> {
        fs::read_dir("fixtures")
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
    async fn test_ipld_stream_get_one_file() {
        let doc_id = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D"
            .try_into()
            .unwrap();

        let mut stream = IpldStream::new(MyProcessor, doc_id);

        while let Some(item) = stream.next().await {
            match item {
                Ok(IpldItem::File(file)) => {
                    assert_eq!(file.data(), b"Hello, world!");
                },
                Ok(IpldItem::Dir(_)) => {
                    panic!("Expected file, got directory");
                },
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    break;
                },
            }
        }
    }

    #[derive(Clone)]
    struct FixturesProcessor;

    #[async_trait]
    impl Processor for FixturesProcessor {
        type Codec = ipld_dagpb::DagPbCodec;

        async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError> {
            let fixtures = &*FIXTURES;
            let data = fixtures.get(&*id).cloned();
            if let Some(vc) = data {
                let r = DagPbCodec::decode_from_slice(&vc).map(|node: PbNode| {
                    let dir = DocDir::new(id, node.links);
                    IpldItem::Dir(dir)
                })?;
                Ok(Some(r))
            } else {
                Ok(None)
            }
        }
    }

    #[tokio::test]
    async fn test_ipld_stream_fixtures() {
        let docs_id = [
            "bafybeibfhhww5bpsu34qs7nz25wp7ve36mcc5mxd5du26sr45bbnjhpkei",
            "bafybeibh647pmxyksmdm24uad6b5f7tx4dhvilzbg2fiqgzll4yek7g7y4",
            "bafybeie7xh3zqqmeedkotykfsnj2pi4sacvvsjq6zddvcff4pq7dvyenhu",
            "bafybeigcsevw74ssldzfwhiijzmg7a35lssfmjkuoj2t5qs5u5aztj47tq",
            "bafybeihyivpglm6o6wrafbe36fp5l67abmewk7i2eob5wacdbhz7as5obe",
        ]
        .iter()
        .map(|s| <DocId as TryFrom<&str>>::try_from(*s).unwrap())
        .collect::<Vec<DocId>>();

        for doc_id in docs_id.iter() {
            test_ipld_stream_get_fixture(doc_id.clone()).await;
        }
    }

    #[allow(unused)]
    async fn test_ipld_stream_get_fixture(doc_id: DocId) {
        let mut stream = IpldStream::new(FixturesProcessor, doc_id);

        while let Some(item) = stream.next().await {
            match item {
                Ok(IpldItem::File(_)) => {
                    panic!("Expected directory, got file");
                },
                Ok(IpldItem::Dir(_)) => {},
                Err(e) => {
                    panic!("Error: {:?}", e);
                },
            }
        }
    }
}
