use std::ops::Deref;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use ipld_core::cid::Cid;
use ipld_core::ipld::Ipld;
use ipld_dagpb::PbNode;
use reqwest::Url;

use super::processor::{IpldStream, Processor};
use crate::decoder::fs::{DocId, IpldItem, Link};
use crate::errors::IpldError;
use crate::unixfs::Data;

/// Processor for DAG-PB nodes in IPFS with UnixFS data
#[derive(Clone)]
pub struct IpldDagPbProcessor {
    ipfs_url: Url,
}

struct PbNodeWrapper(PbNode);

impl From<PbNode> for PbNodeWrapper {
    fn from(node: PbNode) -> Self {
        Self(node)
    }
}

impl From<PbNodeWrapper> for Vec<Link> {
    fn from(node: PbNodeWrapper) -> Self {
        node.links.iter().map(Into::into).collect()
    }
}

impl Deref for PbNodeWrapper {
    type Target = PbNode;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PbNodeWrapper {
    pub fn from_bytes(bytes: &Bytes) -> Result<Self, IpldError> {
        let node = PbNode::from_bytes(bytes.clone())?;
        Ok(node.into())
    }
}

impl IpldDagPbProcessor {
    /// Creates a new `IpldDagPbProcessor` with the given IPFS URL.
    ///
    /// # Arguments
    ///
    /// * `ipfs_url` - A string slice that holds the URL of the IPFS node.
    ///
    /// # Panics
    ///
    /// This function will panic if the provided IPFS URL is invalid.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let processor = IpldDagPbProcessor::new("http://localhost:5001");
    /// ```
    pub fn new(ipfs_url: &str) -> Self {
        Self {
            ipfs_url: Url::parse(ipfs_url)
                .unwrap_or_else(|_| panic!("Invalid IPFS URL {}", ipfs_url)),
        }
    }

    async fn request(&self, cid: &Cid) -> Result<Bytes, IpldError> {
        let url = self.ipfs_url.clone();
        let url = url.join(&format!("ipfs/{}?format=raw", cid))?;
        let response = reqwest::get(url).await?;
        response.bytes().await.map_err(Into::into)
    }

    /// Creates a new `IpldStream` for streaming the content of the given CID.
    ///
    /// # Arguments
    ///
    /// * `cid` - The CID of the content to stream.
    ///
    /// # Returns
    ///
    /// An `IpldStream` for the given CID.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use fleek_ipld::walker::dag_pb::IpldDagPbProcessor;
    /// use ipld_core::cid::Cid;
    /// use tokio_stream::StreamExt as _;
    ///
    /// let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D"
    ///     .try_into()
    ///     .unwrap();
    ///
    /// let mut stream = IpldDagPbProcessor::new("http://localhost:5001").stream(cid);
    ///
    /// while let Some(item) = stream.next().await {
    ///     match item {
    ///         Ok(IpldItem::File(file)) => {
    ///             let file_name = file
    ///                 .id()
    ///                 .link()
    ///                 .name()
    ///                 .clone()
    ///                 .unwrap_or("unknown_file.jpg".to_string());
    ///             let mut f = tokio::fs::File::create(file_name).await?;
    ///             f.write_all(file.data()).await?;
    ///             println!("File data: {:?} \n", file);
    ///         },
    ///         Ok(IpldItem::Dir(dir)) => {
    ///             println!("Dir data: {:?} \n", dir);
    ///         },
    ///         Err(e) => {
    ///             panic!("Error: {:?}", e);
    ///         },
    ///     }
    /// }
    /// ```
    pub fn stream(self, cid: impl Into<Cid>) -> IpldStream<Self> {
        IpldStream::new(self, cid.into().into())
    }
}

#[async_trait]
impl Processor for IpldDagPbProcessor {
    async fn get(&self, doc_id: DocId) -> Result<Option<IpldItem>, IpldError> {
        let bytes = self.request(doc_id.cid()).await?;
        let node = PbNodeWrapper::from_bytes(&bytes)?;
        let ipld = node.clone().into();
        if let Ipld::Map(map) = ipld {
            if let Some(Ipld::Bytes(ty)) = map.get("Data") {
                if *ty == [8, 1] {
                    let item = IpldItem::to_dir(doc_id, node.into());
                    return Ok(Some(item));
                } else if node.links.is_empty() {
                    let data = Data::try_from(&node.data)?;
                    let item = IpldItem::to_file(doc_id, Bytes::copy_from_slice(&data.Data));
                    return Ok(Some(item));
                } else {
                    let data: Bytes = self.get_file_link_data(&doc_id, node).await?;
                    let item = IpldItem::to_file(doc_id, data);
                    return Ok(Some(item));
                }
            } else {
                Err(IpldError::CannotDecodeDagPbData(*doc_id.cid()))
            }
        } else {
            Err(IpldError::CannotDecodeDagPbData(*doc_id.cid()))
        }
    }
}

impl IpldDagPbProcessor {
    async fn get_file_link_data(
        &self,
        cid: &DocId,
        node: PbNodeWrapper,
    ) -> Result<Bytes, IpldError> {
        let mut buf = BytesMut::new();
        for link in &node.links {
            let bytes = self.request(&link.cid).await?;
            let node = PbNodeWrapper::from_bytes(&bytes)?;
            if node.links.is_empty() {
                let data = Data::try_from(&node.data)?;
                buf.extend_from_slice(&data.Data);
            } else {
                let data = Box::pin(self.get_file_link_data(cid, node));
                let data = data.await?;
                buf.extend_from_slice(&data);
            }
        }
        Ok(buf.freeze())
    }
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;
    #[allow(unused_imports)]
    use ipld_core::cid::Cid;
    #[allow(unused_imports)]
    use tokio_stream::StreamExt as _;

    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn test_get() {
        let file = "bafybeigcsevw74ssldzfwhiijzmg7a35lssfmjkuoj2t5qs5u5aztj47tq.dag-pb";
        let file_bytes = tokio::fs::read(format!("tests/fixtures/{}", file))
            .await
            .unwrap();
        let mock_server = MockServer::start_async().await;

        let mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/ipfs/QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D")
                .query_param("format", "raw");
            then.status(200)
                .header("accept", "application/vnd.ipld.raw")
                .header("content-type", "application/vnd.ipld.raw")
                .body(file_bytes);
        });

        let host = mock_server.base_url();
        let cid: Cid = "QmSnuWmxptJZdLJpKRarxBMS2Ju2oANVrgbr2xWbie9b2D"
            .try_into()
            .unwrap();
        let processor = IpldDagPbProcessor::new(&host);
        let item = processor.get(cid.into()).await.unwrap();
        assert!(item.is_some());
        assert!(matches!(item.unwrap(), IpldItem::Dir(_)));
        mock.assert();
    }

    #[tokio::test]
    async fn test_get_with_links() {
        let files = [
            "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi",
            "QmcBLKyRHjbGeLnjnmj74FFJpGJDz4YxFqUDYqMU7Mny1p",
            "QmddrRa6PVSnPTyMRBsPTpqnWTvc8n8kfqdt2iVGx5gv3m",
            "QmRCYNxaJKaXEQEZYbzANjB9uCsiVYrDuY6TNqWtQQDamq",
            "QmaNYRP83ARdjELmoQWTLoJ31vxn3zmBxK3d7vR6gAfiLZ",
            "QmVNz2znpRpwPFafwbb6TJCN7FWrxv6eprQeJLnkA8sDqh",
            "QmdVS3CcQMfJt8XvPcswNWgnQ2mHyk9wjvFSqrYxjFW83u",
            "QmRP87qouYU5AewinSGvxog8d5zEuYonLtM9cTcSL8Rdr6",
            "QmRd5xGHUtY2mNcrb8uSG8VrhcynChqJU9z3oYx2oLcdmJ",
            "QmaEAM7XWkY9P8A4nmBK4qFDGxnEqadmevfkCpoodpjnna",
            "QmQFWxRHjCKjPPZ92GP56cdtw7kCGwP2bNp6poHfmCuh6t",
            "QmciHZfp2BT5yJyEW7U9w5LGPcsxpbPR5aVr4fcTVFdW97",
            "QmWwXc53v3xS1Z8BHmKco8dTnNqQyveU5GdsF7aoRaAKka",
            "QmYemNQVTQseKv4U5EpSbVSJHXtdt6HpHWrNFJtZmYgB4m",
            "QmcoUbH5cDdkjbMXWWq5nw64UJbJQY7NFaGdXzoF4ptGV3",
            "QmRQY4bwaot6wVtJiFHeK4VPYK1Z29BkDfLP9E3nLbTgVn",
            "QmasVpGLWXGDs4aHha7DMLW3JU1b6mNF2VnHozjHiePNTq",
            "QmQ4FawzQY28kUpRNdrCxp78D2jtf8q6n41Se6wvouBUhD",
            "QmNbPfbbvh3qvBjZtWSzxCrx2o6Vc6iV4xvyBf29eqzZZz",
        ];
        let mock_server = MockServer::start_async().await;

        let mocks = files
            .iter()
            .map(|file| {
                let file_bytes = std::fs::read(format!("tests/fixtures/{}.dag-pb", file)).unwrap();
                mock_server.mock(|when, then| {
                    when.method(GET)
                        .path(&format!("/ipfs/{}", file))
                        .query_param("format", "raw");
                    then.status(200)
                        .header("accept", "application/vnd.ipld.raw")
                        .header("content-type", "application/vnd.ipld.raw")
                        .body(file_bytes.clone());
                })
            })
            .collect::<Vec<_>>();

        let host = mock_server.base_url();
        let cid: Cid = "Qmc8mmzycvXnzgwBHokZQd97iWAmtdFMqX4FZUAQ5AQdQi"
            .try_into()
            .unwrap();
        let processor = IpldDagPbProcessor::new(&host);
        let item = processor.get(cid.into()).await.unwrap();
        assert!(item.is_some());
        for mock in mocks {
            mock.assert();
        }
    }
}
