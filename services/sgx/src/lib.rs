use std::fs;
use std::future::Future;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock};

use aesm_client::AesmClient;
use enclave_runner::usercalls::{AsyncStream, UsercallExtension};
use enclave_runner::EnclaveBuilder;
use futures::executor::block_on;
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

static PEER_IPS: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("PEER_IPS")
        .expect("PEER_IPS env variable not found")
        .split(',')
        .map(|s| s.to_string())
        .collect()
});

static OUR_NODE_INDEX: LazyLock<u32> = LazyLock::new(|| {
    std::env::var("OUR_NODE_INDEX")
        .expect("OUR_NODE_INDEX env variable not found")
        .parse()
        .expect("OUR_NODE_INDEX must be a valid integer")
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

pub fn main() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .thread_name("sdk")
            .enable_all()
            .build()
            .expect("failed to build sdk runtime");

        rt.spawn(async move {
            fn_sdk::ipc::init_from_env();
            futures::future::pending::<()>().await
        })
    });

    // Running the enclave
    let aesm_client = AesmClient::new();
    let mut device = IsgxDevice::new()
        .unwrap()
        .einittoken_provider(aesm_client)
        .build();

    let mut enclave_builder = EnclaveBuilder::new_from_memory(ENCLAVE);

    let enclave_args = block_on(get_enclave_args());
    enclave_builder.args(enclave_args);

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

async fn get_enclave_args() -> Vec<Vec<u8>> {
    let node_index = *OUR_NODE_INDEX;
    // First arg is either the sealed key or a list of peers to get it from
    let first_arg = {
        // todo: make a specific spot for this file
        if let Ok(sealed_shared_key) = fs::read(SGX_SEALED_DATA_PATH.join("sealedkey.bin")) {
            let hex_encoded = hex::encode(sealed_shared_key);
            format!("--encoded-secret-key={hex_encoded}")
                .as_bytes()
                .to_vec()
        } else if node_index == 0 {
            // We are the first node to start, so we should generate a new key if we dont have one
            // on disk
            "--initial-node".as_bytes().to_vec()
        } else {
            // We dont have a sealed key saved to disk so we should pass in a list of peers to get
            // it from

            // This returns a random sample of peers shuffled
            let peers = PEER_IPS.clone();
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
