use std::future::Future;
use std::io::Result as IoResult;
use std::sync::atomic::AtomicBool;

use aesm_client::AesmClient;
use arrayref::array_ref;
use enclave_runner::usercalls::{AsyncStream, UsercallExtension};
use enclave_runner::EnclaveBuilder;
use futures::FutureExt;
use sgxs_loaders::isgx::Device as IsgxDevice;

use crate::blockstore::VerifiedStream;

mod blockstore;
mod listener;

const ENCLAVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/enclave.sgxs"));

#[derive(Debug)]
struct ExternalService;

impl UsercallExtension for ExternalService {
    fn connect_stream<'future>(
        &'future self,
        addr: &'future str,
        _local_addr: Option<&'future mut String>,
        _peer_addr: Option<&'future mut String>,
    ) -> std::pin::Pin<Box<dyn Future<Output = IoResult<Option<Box<dyn AsyncStream>>>> + 'future>>
    {
        async move {
            // Connect the enclave to a blockstore content-stream
            if let Some(hash) = addr.strip_suffix(".blockstore.fleek.network") {
                let hash = hex::decode(hash).expect("valid blake3 hex");
                let stream = Box::new(VerifiedStream::new(array_ref![hash, 0, 32]).await?);
                return Ok(Some(stream as _));
            }

            // Otherwise, fallback to default behavior of parsing as an ip address
            Ok(None)
        }
        .boxed_local()
    }

    fn bind_stream<'future>(
        &'future self,
        addr: &'future str,
        _local_addr: Option<&'future mut String>,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = IoResult<Option<Box<dyn enclave_runner::usercalls::AsyncListener>>>>
                + 'future,
        >,
    > {
        async move {
            if addr == "requests.fleek.network" {
                // Bind to request listener. Can only be used once (when enclave starts up).
                static STARTED: AtomicBool = AtomicBool::new(false);
                if STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    return Ok(Some(Box::new(listener::RequestListener::bind().await) as _));
                }
            }

            // Otherwise, fallback to default behavior of binding to a tcp address.
            Ok(None)
        }
        .boxed_local()
    }
}

fn main() {
    fn_sdk::ipc::init_from_env();

    let mut device = IsgxDevice::new()
        .unwrap()
        .einittoken_provider(AesmClient::new())
        .build();

    let mut enclave_builder = EnclaveBuilder::new_from_memory(ENCLAVE);
    enclave_builder.usercall_extension(ExternalService);

    // TODO: figure out a flow to generate a signature for the compiled enclave and committing it.
    enclave_builder.dummy_signature();

    let enclave = enclave_builder.build(&mut device).unwrap();
    if let Err(e) = enclave.run() {
        eprintln!("Error while executing SGX enclave: {e}");
        std::process::exit(1)
    }
}