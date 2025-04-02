use std::fs;
use std::future::Future;
use std::io::{Result as IoResult, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use aesm_client::AesmClient;
use b3fs::bucket::file::writer::FileWriter;
use b3fs::bucket::Bucket;
use enclave_runner::usercalls::{AsyncStream, UsercallExtension};
use enclave_runner::EnclaveBuilder;
use futures::FutureExt;
use rand::random;
use req_res::AttestationEndpoint;
use sgxs_loaders::isgx::Device as IsgxDevice;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use crate::blockstore::VerifiedStream;

mod blockstore;
mod connection;
mod req_res;

mod config {
    /* WASM configuration */

    /// Maximum size of blockstore content. 16 MiB
    pub const MAX_BLOCKSTORE_SIZE: usize = 16 << 20;

    /// Maxmimum fuel limit allowed to be set by the client. 40 billion
    pub const MAX_FUEL_LIMIT: u64 = 10 << 32;

    /// Maximum size of input parameter. 8 MiB
    pub const MAX_INPUT_SIZE: usize = 8 << 20;

    /// Maximum size of wasm output. 16 MiB
    pub const MAX_OUTPUT_SIZE: usize = 16 << 20;

    /// Maximum number of concurrent wasm threads.
    /// Must not exceed threads reserved for enclave (134)
    pub const MAX_CONCURRENT_WASM_THREADS: usize = 128;

    /* TLS server configuration */

    /// TLS key size, must be >= 2048
    pub const TLS_KEY_SIZE: usize = 2048;

    /// MTLS port to listen on for incoming enclave requests
    pub const MTLS_PORT: u16 = 55855;

    /// TLS port to listen on for incoming public key requests
    pub const TLS_PORT: u16 = 55856;
}

const ENCLAVE: &[u8] = include_bytes!("../enclave.sgxs");
//const ENCLAVE: &[u8] =
// include_bytes!("../../../../../lightning-pod-enclave/target/x86_64-fortanix-unknown-sgx/release/
// lightning-pod-enclave.sgxs");

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
                println!("#### connect stream 1");
                // Connect the enclave to a blockstore content-stream
                if let Some(hash) = subdomain.strip_suffix(".blockstore") {
                    println!("#### connect stream 2");
                    let hash = hex::decode(hash).expect("valid blake3 hex");
                    let stream = Box::new(
                        VerifiedStream::new(
                            arrayref::array_ref![hash, 0, 32],
                            self.attest_state.get_blockstore_path(),
                        )
                        .await?,
                    ) as Box<dyn AsyncStream>;
                    println!("#### connect stream 4");
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

pub const HEADER_DIR_VERSION: u32 = 1;
pub const HEADER_FILE_VERSION: u32 = 0;
pub const POSITION_START_HASHES: usize = 8;
pub const POSITION_START_NUM_ENTRIES: usize = POSITION_START_HASHES - 4;
pub const CHUNK_LEN: usize = 1024;

pub const BLOCK_SIZE_IN_CHUNKS: usize = 256;
pub const MAX_BLOCK_SIZE_IN_BYTES: usize = CHUNK_LEN * BLOCK_SIZE_IN_CHUNKS;

pub(crate) fn get_random_file(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(MAX_BLOCK_SIZE_IN_BYTES);
    for _ in 0..size {
        let d: [u8; 32] = random();
        data.extend(d);
    }
    data
}

pub fn main() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .thread_name("sdk")
            .enable_all()
            .build()
            .expect("failed to build sdk runtime");

        rt.block_on(async move {
            fn_sdk::ipc::init_from_env();
            futures::future::pending::<()>().await
        });
    });

    let blockstore_path =
        std::env::var("BLOCKSTORE_PATH").expect("blockstore path env var not set");

    let blockstore_path = PathBuf::from(blockstore_path);
    let blockstore_path_clone = blockstore_path.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .thread_name("test")
            .enable_all()
            .build()
            .expect("failed to build sdk runtime");

        rt.block_on(async move {
            // Put file into bucket
            let n_blocks = 10;
            let bucket = Bucket::open(&blockstore_path_clone).await.unwrap();
            let mut writer = FileWriter::new(&bucket).await.unwrap();
            let data = get_random_file(8192 * n_blocks);
            writer.write(&data).await.unwrap();
            let root_hash = writer.commit().await.unwrap();

            println!("data to send: {}", data.len());

            tokio::time::sleep(Duration::from_secs(15)).await;
            match UnixStream::connect("/home/ubuntu/.lightning/ipc/service-4/conn").await {
                Ok(mut stream) => {
                    println!(">>> connect successfully");
                    if let Err(e) = stream.write_all(&32u32.to_be_bytes()).await {
                        println!(">> error: {e:?}");
                    }
                    if let Err(e) = stream.write_all(&root_hash).await {
                        println!(">> error: {e:?}");
                    }
                    println!(">> write success");
                },
                Err(e) => {
                    println!("error: {e:?}");
                },
            }

            let content_header = bucket.get(&root_hash).await.unwrap();
            let num_blocks = content_header.blocks();

            let file = content_header.into_file().unwrap();
            let mut tree = file.hashtree().await.unwrap();

            let mut data: Vec<u8> = Vec::new();
            for block in 0..num_blocks {
                let hash_ = tree.get_hash(block).await.unwrap().unwrap();
                let chunk = bucket.get_block_content(&hash_).await.unwrap().unwrap();
                data.extend(&chunk);
            }
            println!("DATA read from bucket: {}", data.len());
        });
    });

    // Running the enclave
    let aesm_client = AesmClient::new();
    let mut device = IsgxDevice::new()
        .unwrap()
        .einittoken_provider(aesm_client)
        .build();

    let mut enclave_builder = EnclaveBuilder::new_from_memory(ENCLAVE);

    let enclave_args = get_enclave_args();
    enclave_builder.args(enclave_args);

    // setup attestation state
    let attest_state = Arc::new(
        req_res::EndpointState::init(blockstore_path)
            .expect("failed to initialize attestation endpoint"),
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
    let mut args = vec![];

    let node_index = *OUR_NODE_INDEX;
    // First arg is either the sealed key or a list of peers to get it from
    let first_arg = {
        // todo: make a specific spot for this file
        if let Ok(sealed_shared_key) = fs::read(SGX_SEALED_DATA_PATH.join("sealedkey.bin")) {
            format!("--encoded-secret-key={}", hex::encode(sealed_shared_key)).into()
        } else if node_index == 0 {
            // We are the first node to start, so we should generate a new key if we dont have one
            // on disk
            "--initial-node".into()
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
    args.push(first_arg);

    // todo: actually get this from somewhere
    args.push("--our-ip=127.0.0.1".into());

    // tls configuration
    args.push(format!("--tls-key-size={}", config::TLS_KEY_SIZE).into());
    args.push(format!("--tls-port={}", config::TLS_PORT).into());
    args.push(format!("--mtls-port={}", config::MTLS_PORT).into());

    // wasm configuration
    args.push(format!("--max-blockstore-size={}", config::MAX_BLOCKSTORE_SIZE).into());
    args.push(format!("--max-fuel-limit={}", config::MAX_FUEL_LIMIT).into());
    args.push(format!("--max-input-size={}", config::MAX_INPUT_SIZE).into());
    args.push(format!("--max-output-size={}", config::MAX_OUTPUT_SIZE).into());
    args.push(
        format!(
            "--max-concurrent-wasm-threads={}",
            config::MAX_CONCURRENT_WASM_THREADS
        )
        .into(),
    );

    // enable debug prints if SGX_WASM_DEBUG is defined and not empty
    if let Ok(v) = std::env::var("SGX_WASM_DEBUG") {
        if !v.is_empty() {
            args.push("--debug".as_bytes().to_vec())
        }
    }

    args
}
