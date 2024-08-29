use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures::future::BoxFuture;
use futures::TryFuture;
use ipld_core::cid::Cid;
use ipld_dagpb::PbLink;
use tokio_stream::Stream;

use self::errors::IpldError;

pub mod errors;

#[derive(Clone, Debug)]
pub struct DocId {
    cid: Cid,
    name: Option<String>,
    size: Option<u64>,
}

impl DocId {
    pub fn new(cid: Cid, name: Option<String>, size: Option<u64>) -> Self {
        Self { cid, name, size }
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn size(&self) -> &Option<u64> {
        &self.size
    }
}

impl From<Cid> for DocId {
    fn from(cid: Cid) -> Self {
        Self::new(cid, None, None)
    }
}

#[derive(Clone, Debug)]
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

    pub fn from_pb_node(id: impl Into<DocId>, node: ipld_dagpb::PbNode) -> Self {
        Self {
            id: id.into(),
            links: node.links,
        }
    }

    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn links(&self) -> &Vec<PbLink> {
        &self.links
    }
}

impl From<DocDir> for IpldItem {
    fn from(dir: DocDir) -> Self {
        IpldItem::Dir(dir)
    }
}

#[derive(Clone, Debug)]
pub struct DocFile {
    id: DocId,
    data: Option<Bytes>,
}

impl From<DocFile> for IpldItem {
    fn from(file: DocFile) -> Self {
        IpldItem::File(file)
    }
}

impl DocFile {
    pub fn new(id: DocId, data: Option<Bytes>) -> Self {
        Self {
            id: id.into(),
            data,
        }
    }

    pub fn from_pb_node(id: DocId, node: ipld_dagpb::PbNode) -> Self {
        Self::new(id, node.data)
    }

    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn data(&self) -> &Option<Bytes> {
        &self.data
    }
}

#[derive(Clone, Debug)]
pub enum IpldItem {
    Dir(DocDir),
    File(DocFile),
}

/// A trait for processing IPLD data.
///
/// This trait is used to retrieve IPLD data from a storage backend.
///
/// The associated type `Codec` is the IPLD codec used to encode and decode the data.
///
/// `IpldItem` is the type of item that will be returned by the processor and it is specific to
/// `Dir` and `File` types because, those are going to be used for `b3fs` backend.
#[async_trait]
pub trait Processor {
    /// Get IpldItem from the storage backend.
    async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError>;
}

pub struct IpldStream<P: Processor + Unpin + Send + Sync> {
    processor: P,
    current_id: Option<DocId>,
    next_link: Option<PbLink>,
    current_dir: Option<DocDir>,
    current_links: Vec<PbLink>,
    current_index: usize,
    pending_future: Option<BoxFuture<'static, Result<Option<IpldItem>, IpldError>>>,
}

impl<P> IpldStream<P>
where
    P: Processor + Unpin + Send + Sync,
{
    pub fn new(processor: P, id: DocId) -> Self {
        Self {
            processor,
            current_id: Some(id),
            current_dir: None,
            next_link: None,
            current_links: Vec::new(),
            current_index: 0,
            pending_future: None,
        }
    }

    pub fn current_id(&self) -> Option<DocId> {
        if let Some(link) = &self.next_link {
            Some(DocId::new(link.cid, link.name.clone(), link.size))
        } else {
            self.current_id.clone()
        }
    }
}

/// Implementing Stream for IpldStream in order to traverse all the links associated to a given Cid
impl<P> Stream for IpldStream<P>
where
    P: Processor + Unpin + Send + Sync + Clone + 'static,
{
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
                            if !this.current_links.is_empty() {
                                let link = &this.current_links[this.current_index];
                                this.next_link = Some(link.clone());
                                this.current_id = this.current_id();
                                this.current_index += 1;
                            }
                            return Poll::Ready(Some(Ok(IpldItem::Dir(dir))));
                        },
                        Ok(Some(IpldItem::File(file))) => {
                            return Poll::Ready(Some(Ok(IpldItem::File(file))));
                        },
                        Ok(None) => {
                            return Poll::Ready(None);
                        },
                        Err(e) => {
                            return Poll::Ready(Some(Err(e)));
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
                this.next_link = Some(link.clone());
                this.current_id = this.current_id();
                this.current_index += 1;
                return Poll::Ready(Some(Ok(IpldItem::Dir(dir.clone()))));
            } else {
                this.current_dir = None;
                this.current_links.clear();
                this.next_link = None;
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
        async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError> {
            let data: Bytes = b"Hello, world!".to_vec().into();
            let file = DocFile::new(id, Some(data));
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
        let doc_id: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D"
            .try_into()
            .unwrap();
        let doc_id = doc_id.into();

        let mut stream = IpldStream::new(MyProcessor, doc_id);

        while let Some(item) = stream.next().await {
            match item {
                Ok(IpldItem::File(file)) => {
                    assert_eq!(file.data(), &Some(Bytes::from_static(b"Hello, world!")));
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
        async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError> {
            let fixtures = &*FIXTURES;
            let data = fixtures.get(id.cid()).cloned();
            if let Some(vc) = data {
                let data = DagPbCodec::decode_from_slice(&vc)
                    .map(|node: PbNode| Some(DocDir::from_pb_node(id, node).into()))?;
                Ok(data)
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
        .map(|s| <Cid as TryFrom<&str>>::try_from(*s).unwrap())
        .collect::<Vec<Cid>>();

        for &doc_id in docs_id.iter() {
            test_ipld_stream_get_fixture(doc_id).await;
        }
    }

    #[allow(unused)]
    async fn test_ipld_stream_get_fixture(doc_id: Cid) {
        let mut stream = IpldStream::new(FixturesProcessor, doc_id.into());

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
