use std::fs::File;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::task::Waker;

use aesm_client::AesmClient;
use anyhow::{ensure, Context};
use arrayref::array_ref;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use sgx_isa::Targetinfo;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::SGX_SEALED_DATA_PATH;

const SGX_QL_ALG_ECDSA_P256: u32 = 2;

#[derive(Debug)]
pub struct EndpointState {
    aesm_client: AesmClient,
    target_info: Targetinfo,
    ecdsa_key_id: Vec<u8>,
    blockstore_path: PathBuf,
}

fn get_algorithm_id(key_id: &[u8]) -> u32 {
    const ALGORITHM_OFFSET: usize = 154;
    let mut bytes: [u8; 4] = Default::default();
    bytes.copy_from_slice(&key_id[ALGORITHM_OFFSET..ALGORITHM_OFFSET + 4]);
    u32::from_le_bytes(bytes)
}

impl EndpointState {
    pub fn init(blockstore_path: PathBuf) -> anyhow::Result<Self> {
        let aesm_client = AesmClient::new();
        let key_ids = aesm_client.get_supported_att_key_ids().unwrap();
        println!("got key ids");
        let ecdsa_key_ids: Vec<_> = key_ids
            .into_iter()
            .filter(|id| SGX_QL_ALG_ECDSA_P256 == get_algorithm_id(id))
            .collect();
        ensure!(
            ecdsa_key_ids.len() == 1,
            "Expected exactly one ECDSA attestation key, got {} key(s) instead",
            ecdsa_key_ids.len()
        );
        let ecdsa_key_id = ecdsa_key_ids[0].to_vec();
        let quote_info = aesm_client.init_quote_ex(ecdsa_key_id.clone())?;
        println!("got quote info");

        let target_info =
            Targetinfo::try_copy_from(quote_info.target_info()).context("Invalid target info")?;
        println!("got target info");
        Ok(EndpointState {
            aesm_client,
            target_info,
            ecdsa_key_id,
            blockstore_path,
        })
    }

    /// Handle an incoming request from the enclave
    fn handle(self: Arc<Self>, request: Request) -> std::io::Result<Bytes> {
        match request {
            Request::TargetInfo => self.handle_target_info(),
            Request::Quote(report) => self.handle_quote(report),
            Request::Collateral(quote) => self.handle_collateral(quote),
            Request::SaveKey(data) => self.handle_save_key(data),
        }
    }

    /// Handle getting the target info
    pub fn handle_target_info(&self) -> std::io::Result<Bytes> {
        let output = serde_json::to_vec(&self.target_info)?;
        Ok(output.into())
    }

    /// Handle getting a quote from a report
    ///
    /// # Security
    ///
    /// The nonce is set to 0. While in some instances reusing nonces can lead to security
    /// vulnerability, this is not the case here.
    ///
    /// Here is an extract from Intel code [1]
    /// > The caller can request a REPORT from the QE using a supplied nonce. This will allow
    /// > the enclave requesting the quote to verify the QE used to generate the quote. This
    /// > makes it more difficult for something to spoof a QE and allows the app enclave to
    /// > catch it earlier. But since the authenticity of the QE lies in the knowledge of the
    /// > Quote signing key, such spoofing will ultimately be detected by the quote verifier.
    /// > QE REPORT.ReportData = SHA256(*p_{nonce}||*p_{quote})||0x00)
    ///
    /// Since setting a nonce would add no measurable security benefit in our threat model,
    /// we chose not to do so, because it would only add complexity.
    ///
    /// [1] <https://github.com/intel/linux-sgx/blob/26c458905b72e66db7ac1feae04b43461ce1b76f/common/inc/sgx_uae_quote_ex.h#L158>
    pub fn handle_quote(&self, report: Vec<u8>) -> std::io::Result<Bytes> {
        self.aesm_client
            .get_quote_ex(self.ecdsa_key_id.clone(), report, None, vec![0; 16])
            .map(|res| res.quote().to_vec().into())
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("failed to generate quote: {e}"),
                )
            })
    }

    pub fn handle_collateral(&self, quote: Vec<u8>) -> std::io::Result<Bytes> {
        let collat = dcap_quoteprov::SgxQlQveCollateral::new(&quote)?;
        let bytes = serde_json::to_vec(&collat)?;
        Ok(bytes.into())
    }

    pub fn handle_save_key(&self, data: Vec<u8>) -> std::io::Result<Bytes> {
        let state_pub_key = block_on(async move { fn_sdk::api::fetch_sgx_shared_pub_key().await });
        if let Some(state_pub_key) = state_pub_key {
            let pub_key_bytes = &data[data.len() - 33..];
            let pub_key_hex = hex::encode(pub_key_bytes);
            if state_pub_key != pub_key_hex {
                panic!("State public key doesn't match enclave public key");
            }
        }
        let enc_seal_key = &data[..data.len() - 33];
        std::fs::create_dir_all(SGX_SEALED_DATA_PATH.deref())?;
        let mut file = File::create(SGX_SEALED_DATA_PATH.join("sealedkey.bin"))
            .expect("Failed to create file");
        file.write_all(enc_seal_key)?;
        // no need to respond with anything
        Ok(Bytes::new())
    }

    pub fn get_blockstore_path(&self) -> &Path {
        &self.blockstore_path
    }
}

#[derive(Serialize, Deserialize)]
enum Request {
    TargetInfo,
    Quote(Vec<u8>),
    Collateral(Vec<u8>),
    SaveKey(Vec<u8>),
}

pub struct AttestationEndpoint {
    method: Box<str>,
    // input buffer
    buffer: BytesMut,
    // output buffer
    output: Option<Bytes>,
    // quote provider stuff
    state: Arc<EndpointState>,
    waker: Option<Waker>,
}

impl AttestationEndpoint {
    pub fn new(method: &str, state: Arc<EndpointState>) -> Self {
        let mut output = None;
        if method == "target_info" {
            match state.handle_target_info() {
                Ok(b) => output = Some(b),
                Err(_) => unreachable!(),
            }
        }

        Self {
            method: method.into(),
            state,
            buffer: BytesMut::new(),
            output,
            waker: None,
        }
    }
}

impl AsyncWrite for AttestationEndpoint {
    /// Write bytes into the buffer, and attempt to use them,
    /// returning for more bytes if needed.
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.output.is_none() {
            // push bytes into buffer
            self.buffer.put_slice(buf);

            // return for more bytes if needed
            if self.buffer.len() < 4 {
                return std::task::Poll::Ready(Ok(buf.len()));
            }

            // read len delim
            let len = u32::from_be_bytes(*array_ref![self.buffer, 0, 4]) as usize;

            // return for more bytes if needed
            if self.buffer.len() < len + 4 {
                return std::task::Poll::Ready(Ok(buf.len()));
            }

            // parse request payload
            self.buffer.advance(4);

            // set handler future to be polled from `AsyncRead`
            let req = match self.method.as_ref() {
                "quote" => Request::Quote(self.buffer.split().to_vec()),
                "collateral" => Request::Collateral(self.buffer.split().to_vec()),
                "put_key" => Request::SaveKey(self.buffer.split().to_vec()),
                _ => unreachable!("handler checks methods, target info already set output"),
            };

            match self.state.clone().handle(req) {
                Ok(b) => {
                    self.output = Some(b);
                    if let Some(waker) = self.waker.take() {
                        // wake reader if it's already been polled
                        waker.wake()
                    }
                },
                Err(e) => eprintln!("failed to handle attestation request: {e}"),
            }
        }

        std::task::Poll::Ready(Ok(buf.len()))
    }

    /// no-op
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    /// no-op
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncRead for AttestationEndpoint {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> std::task::Poll<std::io::Result<()>> {
        // flush all output
        if let Some(output) = self.output.as_mut() {
            let len = buf.remaining().min(output.len());
            buf.put_slice(&output.split_to(len));
            std::task::Poll::Ready(Ok(()))
        } else {
            self.waker = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}
