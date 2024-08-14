use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use serde::Deserialize;

mod blockstore;
mod runtime;

#[derive(Deserialize)]
struct Request {
    /// Blake3 hash of the wasm module.
    hash: String,
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
    } = serde_json::from_slice(&payload)?;

    // fetch content from blockstore
    let content = blockstore::get_verified_content(&hash)?;

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

    loop {
        let (mut conn, _) = listener.accept()?;
        if let Err(e) = handle_connection(&mut conn) {
            // TODO: write error to stream
            eprintln!("Failed to handle connection: {e}");
        }
    }
}
