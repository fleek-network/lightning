use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::LazyLock;

use ecies::{PublicKey, SecretKey};
use serde::Deserialize;

mod blockstore;
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
    let content = {
        let bytes = blockstore::get_verified_content(&hash)?;
        if decrypt {
            ecies::decrypt(&SHARED_KEY.serialize(), &bytes)?
        } else {
            bytes
        }
    };

    // run wasm module
    let response = runtime::execute_module(content, &function, input)?;

    // write length delimiter and wasm output
    conn.write_all(&(response.len() as u32).to_be_bytes())?;
    conn.write_all(&response)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    // bind to userspace address for incoming requests from handshake
    let listener = TcpListener::bind("requests.fleek.network")?;

    println!("Successfully started SGX enclave!");
    println!(
        "Shared enclave public key: {}",
        bs58::encode(PublicKey::from_secret_key(&SHARED_KEY).serialize_compressed()).into_string()
    );

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
