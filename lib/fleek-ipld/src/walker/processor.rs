use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures::future::BoxFuture;
use futures::TryFuture;
use ipld_core::cid::Cid;
use tokio_stream::Stream;

use crate::errors::IpldError;

/// A link to another IPLD node.
#[derive(Clone, Debug)]
pub struct Link {
    cid: Cid,
    name: Option<String>,
    size: Option<u64>,
}

impl From<Cid> for Link {
    fn from(cid: Cid) -> Self {
        Self::new(cid, None, None)
    }
}

impl Link {
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

/// A unique identifier for an IPLD node, which contains a `Link` and a `PathBuf` with the path to
/// this document. If `PathBuf` is empty, then this is the root document.
#[derive(Clone, Debug)]
pub struct DocId {
    link: Link,
    path: PathBuf,
}

impl DocId {
    pub fn new(link: Link, path: PathBuf) -> Self {
        Self { link, path }
    }

    pub fn cid(&self) -> &Cid {
        self.link.cid()
    }

    pub fn link(&self) -> &Link {
        &self.link
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn from_link(link: &Link, parent: &Option<DirItem>) -> Self {
        let mut path = parent
            .as_ref()
            .map(|dir| dir.id.path.clone())
            .unwrap_or_default();
        if let Some(n) = link.name.as_ref() {
            path.push(n.clone())
        }
        Self::new(link.clone(), path)
    }
}

impl From<Cid> for DocId {
    fn from(cid: Cid) -> Self {
        Self::new(cid.into(), PathBuf::new())
    }
}

/// `DirItem` represents a directory in the IPLD UnixFS data model.
#[derive(Clone, Debug)]
pub struct DirItem {
    id: DocId,
    links: Vec<Link>,
}

impl From<DirItem> for IpldItem {
    fn from(dir: DirItem) -> Self {
        Self::Dir(dir)
    }
}

impl DirItem {
    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn links(&self) -> &Vec<Link> {
        &self.links
    }
}

/// `FileItem` represents a file in the IPLD UnixFS data model.
#[derive(Clone)]
pub struct FileItem {
    id: DocId,
    data: Bytes,
}

impl std::fmt::Debug for FileItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileItem")
            .field("id", &self.id)
            .field("data-length", &self.data.len())
            .finish()
    }
}

impl From<FileItem> for IpldItem {
    fn from(file: FileItem) -> Self {
        Self::File(file)
    }
}

impl FileItem {
    pub fn id(&self) -> &DocId {
        &self.id
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }
}

#[derive(Clone, Debug)]
pub enum IpldItem {
    Dir(DirItem),
    File(FileItem),
}

impl IpldItem {
    pub fn from_dir(id: DocId, links: Vec<Link>) -> Self {
        let dir = DirItem { id, links };
        Self::Dir(dir)
    }

    pub fn from_file(id: DocId, data: Option<Bytes>) -> Self {
        let data = data.unwrap_or_default();
        let file = FileItem { id, data };
        Self::File(file)
    }

    pub fn is_dir(&self) -> bool {
        match self {
            Self::Dir(_) => true,
            Self::File(_) => false,
        }
    }

    pub fn id(&self) -> &DocId {
        match self {
            Self::Dir(dir) => &dir.id,
            Self::File(file) => &file.id,
        }
    }
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
    stack: Vec<(Option<DirItem>, Vec<Link>, usize)>,
    pending_future: Option<BoxFuture<'static, Result<Option<IpldItem>, IpldError>>>,
}

impl<P> IpldStream<P>
where
    P: Processor + Unpin + Send + Sync,
{
    pub fn new(processor: P, id: DocId) -> Self {
        Self {
            processor,
            stack: vec![(None, vec![id.link().clone()], 0)],
            pending_future: None,
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
                        Ok(Some(ref i @ IpldItem::Dir(ref item))) => {
                            let links = item.links.clone();
                            this.stack.push((Some(item.clone()), links, 0));
                            return Poll::Ready(Some(Ok(i.clone())));
                        },
                        Ok(Some(item @ IpldItem::File(_))) => {
                            return Poll::Ready(Some(Ok(item)));
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

        while let Some((current_dir, current_links, current_index)) = this.stack.last_mut() {
            if *current_index < current_links.len() {
                let link = &current_links[*current_index];
                *current_index += 1;
                let processor = this.processor.clone();
                let cid = DocId::from_link(link, current_dir);
                this.pending_future = Some(Box::pin(async move { processor.get(cid).await }));
                cx.waker().wake_by_ref();
                return Poll::Pending;
            } else {
                this.stack.pop();
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

    #[derive(Clone, Default)]
    #[allow(dead_code)]
    struct MyProcessor;

    #[async_trait]
    impl Processor for MyProcessor {
        async fn get(&self, id: DocId) -> Result<Option<IpldItem>, IpldError> {
            let data: Bytes = b"Hello, world!".to_vec().into();
            let file = IpldItem::from_file(id, Some(data));
            Ok(Some(file))
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
                Ok(IpldItem::File(f)) => {
                    assert_eq!(f.data(), &Bytes::from_static(b"Hello, world!"));
                },
                Ok(_) => {
                    panic!("Expected file, got dir");
                },
                Err(e) => {
                    panic!("Error: {:?}", e);
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
                let data = DagPbCodec::decode_from_slice(&vc).map(|node: PbNode| {
                    let links = node
                        .links
                        .into_iter()
                        .map(|link| Link::new(link.cid, link.name, link.size))
                        .collect();
                    Some(IpldItem::from_dir(id, links))
                })?;
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
                    panic!("Expected dir, got file");
                },
                Ok(IpldItem::Dir(dir)) => {
                    assert_eq!(dir.id().cid(), &doc_id);
                },
                Err(e) => {
                    panic!("Error: {:?}", e);
                },
            }
        }
    }
}
