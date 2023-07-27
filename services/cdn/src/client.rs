use anyhow::{anyhow, Result};
use blake3_tree::{blake3::tree::BlockHasher, IncrementalVerifier};
use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::{ClientPublicKey, ClientSignature};
use freek_handshake::client::HandshakeClient;
use freek_interfaces::Blake3Hash;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::connection::{consts::RESPONSE_BLOCK_TAG, CdnConnection, CdnFrame, ServiceMode};

pub struct CdnClient<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    conn: CdnConnection<R, W>,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> CdnClient<R, W> {
    pub async fn new(reader: R, writer: W, pubkey: ClientPublicKey) -> Result<Self> {
        let mut handshake_client = HandshakeClient::new(reader, writer, pubkey, 0.into());
        handshake_client.handshake().await?;
        let (r, w) = handshake_client.request(0).await?;
        let conn = CdnConnection::new(r, w);
        Ok(Self { conn })
    }

    pub async fn request(
        &mut self,
        service_mode: ServiceMode,
        hash: [u8; 32],
    ) -> Result<ResponseIterator<'_, R, W>> {
        // Send request
        self.conn
            .write_frame(CdnFrame::Request { service_mode, hash })
            .await?;

        Ok(ResponseIterator::new(&mut self.conn, service_mode, hash, 0))
    }
}

pub struct ResponseIterator<'a, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    conn: &'a mut CdnConnection<R, W>,
    verifier: IncrementalVerifier,
    block: usize,
    mode: ServiceMode,
}

impl<'a, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> ResponseIterator<'a, R, W> {
    #[inline(always)]
    fn new(
        conn: &'a mut CdnConnection<R, W>,
        mode: ServiceMode,
        root_hash: Blake3Hash,
        start_block: usize,
    ) -> Self {
        Self {
            conn,
            mode,
            verifier: IncrementalVerifier::new(root_hash, start_block),
            block: start_block,
        }
    }

    pub async fn next(&mut self) -> Result<Option<Bytes>> {
        if self.verifier.is_done() {
            return Ok(None);
        }

        let mut buf = BytesMut::with_capacity(256 * 1024);

        // Recieve response
        match self.conn.read_frame(Some(RESPONSE_BLOCK_TAG)).await? {
            Some(CdnFrame::ResponseBlock {
                // TODO: Implement compression support
                compression: _,
                bytes_len,
                proof_len,
            }) => {
                match self.mode {
                    ServiceMode::Tentative => todo!(),
                    ServiceMode::Optimistic => {
                        // Receive proof
                        if proof_len != 0 {
                            self.conn.read_buffer(proof_len as usize);
                            match self.conn.read_frame(None).await? {
                                Some(CdnFrame::Buffer(bytes)) => {
                                    // Feed the verifier the proof
                                    if let Err(e) = self.verifier.feed_proof(&bytes) {
                                        return Err(anyhow!("error feeding proof: {e:?}"));
                                    }
                                },
                                Some(_) => unreachable!(), // Guaranteed by read_buffer()
                                None => {
                                    return Err(anyhow!(
                                        "connection disconnected waiting for proof buffer"
                                    ));
                                },
                            }
                        }

                        // Recieve block
                        self.conn.read_buffer(bytes_len as usize);
                        match self.conn.read_frame(None).await? {
                            Some(CdnFrame::Buffer(bytes)) => {
                                // Verify raw data chunk
                                let mut hasher = BlockHasher::new();
                                hasher.set_block(self.block);
                                hasher.update(&bytes);
                                if let Err(e) = self.verifier.verify(hasher) {
                                    return Err(anyhow!("error verifying content: {e:?}"));
                                }

                                buf.put(bytes);
                            },
                            Some(_) => unreachable!(), // Guaranteed by read_buffer()
                            None => {
                                return Err(anyhow!(
                                    "connection disconnected waiting for byte buffer"
                                ));
                            },
                        }

                        // Send delivery acknowledgement
                        self.conn
                            .write_frame(CdnFrame::DeliveryAcknowledgement {
                                signature: ClientSignature,
                            })
                            .await?;
                    },
                }

                self.block += 1;
            },
            Some(_) => unreachable!(),
            None => {
                return Err(anyhow!(
                    "connection disconnected waiting for response block"
                ));
            },
        }

        Ok(Some(buf.into()))
    }
}
