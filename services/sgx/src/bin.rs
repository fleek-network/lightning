use std::future::Future;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::LazyLock;

use aesm_client::AesmClient;
use enclave_runner::usercalls::{AsyncStream, UsercallExtension};
use enclave_runner::EnclaveBuilder;
use futures::FutureExt;
use sgxs_loaders::isgx::Device as IsgxDevice;

use crate::blockstore::VerifiedStream;

mod blockstore;
mod connection;
mod tls;

static BLOCKSTORE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("BLOCKSTORE_PATH")
        .expect("BLOCKSTORE_PATH env variable not found")
        .into()
});
static IPC_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("IPC_PATH")
        .expect("IPC_PATH env variable not found")
        .into()
});

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
                let stream =
                    Box::new(VerifiedStream::new(arrayref::array_ref![hash, 0, 32]).await?);
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
                if !STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    return Ok(Some(
                        Box::new(connection::ConnectionListener::bind().await) as _
                    ));
                }
            }

            // Otherwise, fallback to default behavior of binding to a tcp address.
            Ok(None)
        }
        .boxed_local()
    }
}

fn main() {
    let mut device = IsgxDevice::new()
        .unwrap()
        .einittoken_provider(AesmClient::new())
        .build();

    let mut enclave_builder = EnclaveBuilder::new_from_memory(ENCLAVE);
    // TODO: figure out a flow to generate a signature for the compiled enclave and committing it.
    enclave_builder.dummy_signature();
    enclave_builder.usercall_extension(ExternalService);
    let enclave = enclave_builder.build(&mut device).unwrap();

    enclave
        .run()
        .map_err(|e| {
            println!("Error while executing SGX enclave.\n{}", e);
            std::process::exit(1)
        })
        .unwrap();
}
