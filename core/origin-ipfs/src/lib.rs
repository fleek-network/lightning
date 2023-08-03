use std::{
    io::{self, ErrorKind},
    pin::Pin,
    task::Poll,
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use hyper::{client, Body, Request, Uri};
use hyper_rustls::ConfigBuilderExt;
use lightning_interfaces::{OriginProviderInterface, UntrustedStream};
use tokio::time::timeout;

const GATEWAY_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct IPFSOrigin {
    gateways: Vec<String>,
}

#[async_trait]
impl OriginProviderInterface<IPFSStream> for IPFSOrigin {
    async fn fetch(&self, uri: &[u8]) -> anyhow::Result<IPFSStream> {
        let requested_cid = Cid::try_from(uri).with_context(|| "Failed to parse uri into cid")?;

        for gateway in self.gateways.iter() {
            let url = Uri::builder()
                .scheme("https")
                .authority(gateway.as_str())
                .path_and_query(format!("/ipfs/{requested_cid}"))
                .build()?;

            // TODO(matthias): we should probably log the error returned from IPFSStream::fetch
            // before moving on to the next gateway.
            if let Ok(Ok(stream)) =
                timeout(GATEWAY_TIMEOUT, IPFSStream::fetch(url, requested_cid)).await
            {
                return Ok(stream);
            }
        }
        Err(anyhow::anyhow!("No response from gateways."))
    }
}

impl Default for IPFSOrigin {
    fn default() -> Self {
        Self {
            gateways: vec![
                "gateway.ipfs.io".to_string(),
                "fleek.ipfs.io".to_string(),
                "ipfs.runfission.com".to_string(),
            ],
        }
    }
}

#[derive(Debug)]
pub struct IPFSStream {
    body: Body,
    requested_cid: Cid,
    data: Vec<u8>,
    done: bool,
}

impl IPFSStream {
    pub async fn fetch(url: Uri, requested_cid: Cid) -> anyhow::Result<Self> {
        // Prepare the TLS client config
        let tls = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_native_roots()
            .with_no_client_auth();

        // Prepare the HTTPS connector
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls)
            .https_or_http()
            .enable_http1()
            .build();

        // Build the hyper client from the HTTPS connector.
        let client: client::Client<_, hyper::Body> = client::Client::builder().build(https);

        let req = Request::builder()
            .uri(url)
            .header("Accept", "application/vnd.ipld.raw")
            .header("Connection", "keep-alive")
            .body(Body::default())?;

        let res = client.request(req).await.map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Could not fetch from IPFS gateway: {e:?}"),
            )
        })?;

        let body = res.into_body();
        Ok(IPFSStream {
            body,
            requested_cid,
            data: Vec::new(),
            done: false,
        })
    }
}

impl tokio_stream::Stream for IPFSStream {
    type Item = Result<bytes::Bytes, io::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let body = Pin::new(&mut self.body);
        match body.poll_next(cx) {
            Poll::Ready(data) => match data {
                Some(Ok(bytes)) => {
                    self.data.extend(bytes.as_ref());
                    Poll::Ready(Some(Ok(bytes)))
                },
                Some(Err(err)) => {
                    Poll::Ready(Some(Err(io::Error::new(ErrorKind::Other, Box::new(err)))))
                },
                None => {
                    self.done = true;
                    Poll::Ready(None)
                },
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl UntrustedStream for IPFSStream {
    fn was_content_valid(&self) -> Option<bool> {
        if !self.done {
            return None;
        }
        match Code::try_from(self.requested_cid.hash().code()) {
            Ok(hasher) => Some(&hasher.digest(&self.data) == self.requested_cid.hash()),
            _ => Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use cid::{
        multihash::{Code, MultihashDigest},
        Cid,
    };
    use lightning_interfaces::OriginProviderInterface;
    use tokio::io::AsyncReadExt;
    use tokio_util::io::StreamReader;

    use crate::IPFSOrigin;

    #[tokio::test]
    async fn test_origin() {
        let req_cid =
            Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
        let ipfs_origin = IPFSOrigin::default();
        let stream = ipfs_origin.fetch(&req_cid.to_bytes()).await.unwrap();

        let mut read = StreamReader::new(stream);
        let mut buf = [0; 1024];
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            let len = read.read(&mut buf).await.unwrap();
            bytes.extend(&buf[..len]);
            if len == 0 {
                break;
            }
        }
        assert!(
            Code::try_from(req_cid.hash().code())
                .ok()
                .map(|code| &code.digest(&bytes) == req_cid.hash())
                .unwrap()
        );
    }
}
