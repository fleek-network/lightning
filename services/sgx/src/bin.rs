use std::fs;
use std::future::Future;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock};

use aesm_client::AesmClient;
use enclave_runner::usercalls::{AsyncStream, UsercallExtension};
use enclave_runner::EnclaveBuilder;
use futures::FutureExt;
use req_res::AttestationEndpoint;
use sgxs_loaders::isgx::Device as IsgxDevice;

use crate::blockstore::VerifiedStream;

mod blockstore;
mod connection;
mod req_res;

static BLOCKSTORE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("BLOCKSTORE_PATH")
        //.expect("BLOCKSTORE_PATH env variable not found")
        .unwrap_or("blockstore_path".to_string())
        .into()
});
static IPC_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("IPC_PATH")
        //.expect("IPC_PATH env variable not found")
        .unwrap_or(String::from("./ipc_path"))
        .into()
});
static SGX_SEALED_DATA_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("SGX_SEALED_DATA_PATH")
        //.expect("SGX_SEALED_DATA_PATH env variable not found")
        .unwrap_or(String::from("./sgx_sealed_data"))
        .into()
});

const ENCLAVE: &[u8] = include_bytes!("../enclave.sgxs");

#[derive(Debug)]
struct ExternalService {
    attest_state: Arc<req_res::EndpointState>,
}

impl UsercallExtension for ExternalService {
    fn connect_stream<'future>(
        &'future self,
        addr: &'future str,
        _local_addr: Option<&'future mut String>,
        _peer_addr: Option<&'future mut String>,
    ) -> std::pin::Pin<Box<dyn Future<Output = IoResult<Option<Box<dyn AsyncStream>>>> + 'future>>
    {
        async move {
            if let Some(subdomain) = addr.strip_suffix(".fleek.network") {
                // Connect the enclave to a blockstore content-stream
                if let Some(hash) = subdomain.strip_suffix(".blockstore") {
                    let hash = hex::decode(hash).expect("valid blake3 hex");
                    let stream =
                        Box::new(VerifiedStream::new(arrayref::array_ref![hash, 0, 32]).await?)
                            as Box<dyn AsyncStream>;
                    return Ok(Some(stream));
                }

                // Attestation APIs
                if let Some(method) = subdomain.strip_suffix(".reqres") {
                    match method {
                        "target_info" | "quote" | "collateral" | "put_key" => {
                            println!("handle {method} endpoint");
                            let stream = Box::new(AttestationEndpoint::new(
                                method,
                                self.attest_state.clone(),
                            )) as Box<dyn AsyncStream>;
                            return Ok(Some(stream));
                        },
                        _ => {},
                    }
                }
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
    // Running the enclave
    let aesm_client = AesmClient::new();
    let mut device = IsgxDevice::new()
        .unwrap()
        .einittoken_provider(aesm_client)
        .build();

    let mut enclave_builder = EnclaveBuilder::new_from_memory(ENCLAVE);

    enclave_builder.args(get_enclave_args());

    // setup attestation state
    let attest_state = Arc::new(
        req_res::EndpointState::init().expect("failed to initialize attestation endpoint"),
    );
    println!("initialized attestation endpoint");

    // TODO: figure out a flow to generate a signature for the compiled enclave and committing it.
    enclave_builder.dummy_signature();
    enclave_builder.usercall_extension(ExternalService { attest_state });
    let enclave = enclave_builder.build(&mut device).unwrap();

    enclave
        .run()
        .map_err(|e| {
            println!("Error while executing SGX enclave.\n{}", e);
            std::process::exit(1)
        })
        .unwrap();
}

fn get_enclave_args() -> Vec<Vec<u8>> {
    // First arg is either the sealed key or a list of peers to get it from
    let first_arg = {
        // todo: make a specific spot for this file
        if let Ok(sealed_shared_key) = fs::read(SGX_SEALED_DATA_PATH.join("sealedkey.bin")) {
            let hex_encoded = hex::encode(sealed_shared_key);
            format!("--encoded-secret-key={hex_encoded}")
                .as_bytes()
                .to_vec()
        } else {
            // We dont have a sealed key saved to disk so we should pass in a list of peers to get
            // it from
            let peers = get_peer_ips();
            let mut arg = "--peer-ips=".as_bytes().to_vec();
            arg.extend_from_slice(peers.join(",").as_bytes());
            arg
        }
    };
    // todo: actually get this from somewhere
    let our_ip = "127.0.0.1";

    let mut our_ip_arg = "--our-ip".as_bytes().to_vec();
    our_ip_arg.extend_from_slice(our_ip.as_bytes());

    vec![first_arg, our_ip_arg]
}

fn get_peer_ips() -> Vec<String> {
    // todo: get this using query runner
    vec!["127.0.0.1".to_string(), "127.0.0.2".to_string()]
}
