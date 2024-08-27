use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::LazyLock;

use ecies::{PublicKey, SecretKey};
use http::start_http_thread;
use serde::Deserialize;

mod attest;
mod blockstore;
mod http;
mod runtime;

pub(crate) mod config {
    pub const MAX_OUTPUT_SIZE: usize = 16 << 20; // 16 MiB
}

/// TODO: Dummy shared key, to be replaced with the key sharing protocol.
/// Public key: 27fjvoWaGcupCpT9ZMfok4gAHGcUhuFt1wgpoVjb4Bhka
static SHARED_KEY: LazyLock<SecretKey> =
    LazyLock::new(|| SecretKey::parse(b"_fleek_dummy_global_network_key_").unwrap());

#[derive(Deserialize)]
struct Request {
    /// Blake3 hash of the wasm module.
    hash: String,
    /// Optionally enable decrypting the wasm file
    #[serde(default)]
    decrypt: bool,
    /// Entrypoint function to call. Defaults to `main`.
    #[serde(default = "default_function_name")]
    function: String,
    /// Input data string.
    #[serde(default)]
    input: String,
}

fn default_function_name() -> String {
    "main".into()
}

fn handle_connection(conn: &mut TcpStream) -> anyhow::Result<()> {
    println!("handling connection in enclave");

    // read length delimiter
    let mut buf = [0; 4];
    conn.read_exact(&mut buf)?;
    let len = u32::from_be_bytes(buf);

    // read payload
    let mut payload = vec![0; len as usize];
    conn.read_exact(&mut payload)?;

    // parse payload
    let Request {
        hash,
        function,
        input,
        decrypt,
    } = serde_json::from_slice(&payload)?;

    // fetch content from blockstore
    let mut module = blockstore::get_verified_content(&hash)?;

    // optionally decrypt the module
    if decrypt {
        module = ecies::decrypt(&SHARED_KEY.serialize(), &module)?;
    }

    // run wasm module
    let output = runtime::execute_module(module, &function, input)?;

    // TODO: Response encodings
    //       - For http: send hash, proof, signature via headers, stream payload in response body.
    //         Should we also allow setting the content-type header from the wasm module?
    //         - X-FLEEK-SGX-OUTPUT-HASH: hex encoded
    //         - X-FLEEK-SGX-OUTPUT-TREE: base64
    //         - X-FLEEK-SGX-OUTPUT-SIGNATURE: base64
    //       - For all others: send hash, signature, then verified b3 stream of content

    // temporary: write wasm output directly
    conn.write_all(&(output.payload.len() as u32).to_be_bytes())?;
    conn.write_all(&output.payload)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("Successfully started SGX enclave!");

    // TODO: - read a list of input node ips
    //       - attempt to fetch the key via RA-TLS from any node

    let shared_key = PublicKey::from_secret_key(&SHARED_KEY).serialize_compressed();
    println!(
        "Shared enclave public key: {}",
        bs58::encode(&shared_key).into_string()
    );

    // Report data [0..33] contains the shared public key
    let mut report_data = [0u8; 64];
    report_data[..33].copy_from_slice(&shared_key);
    start_http_thread(6969, report_data);

    // TODO: spin up RA-TLS server

    // bind to userspace address for incoming requests from handshake
    let listener = TcpListener::bind("requests.fleek.network")?;

    // Handle incoming handshake connections
    loop {
        let (mut conn, _) = listener.accept()?;
        if let Err(e) = handle_connection(&mut conn) {
            // TODO: write error to stream
            let error = format!("Error: {e}");
            eprintln!("{error}");
            let _ = conn.write_all(&(error.len() as u32).to_be_bytes());
            let _ = conn.write_all(error.as_bytes());
        }
    }
}
