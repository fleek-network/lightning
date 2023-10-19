mod stream;

use fn_sdk::api::Origin;
use stream::ServiceStream;
use tokio::net::UnixStream;
use tracing::trace;

pub async fn main() {
    fn_sdk::ipc::init_from_env();
    println!("Initialized IPFS fetcher service!");

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok((socket, _)) = listener.accept().await {
        tokio::spawn(connection_loop(socket));
    }
}

pub async fn connection_loop(socket: UnixStream) {
    let mut stream = ServiceStream::new(socket);

    while let Some(uri) = stream.recv().await {
        // Fetch the content from the origin
        let Some(hash) = fn_sdk::api::fetch_from_origin(Origin::IPFS, uri).await else {
            println!("failed to fetch from origin");
            return
        };

        // Get the content from the blockstore
        let Ok(handle) = fn_sdk::blockstore::ContentHandle::load(&hash).await else {
            eprintln!("failed to load content handle from the blockstore");
            return
        };

        trace!("streaming {} blocks to the client", handle.len());

        let bytes = (handle.len() as u32).to_be_bytes();
        if let Err(e) = stream.send(bytes.as_slice()).await {
            eprintln!("failed to send number of blocks: {e}");
            return;
        }

        for block in 0..handle.len() {
            let Ok(bytes) = handle.read(block).await else {
                eprintln!("failed to read content from the blockstore :(");
                return
            };

            if let Err(e) = stream.send(&bytes).await {
                eprintln!("failed to send block: {e}");
                return;
            }
        }
    }
}
