mod stream;

use fn_sdk::api::Origin;
use fn_sdk::connection::Connection;
use stream::ServiceStream;
use tracing::{info, trace};

pub async fn main() {
    fn_sdk::ipc::init_from_env();
    info!("Initialized IPFS fetcher service!");

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(connection_loop(conn));
    }
}

pub async fn connection_loop(conn: Connection) {
    let mut stream = ServiceStream::new(conn);

    while let Some(uri) = stream.read_request().await {
        // Fetch the content from the origin
        let Some(hash) = fn_sdk::api::fetch_from_origin(Origin::IPFS, uri).await else {
            println!("failed to fetch from origin");
            return
        };

        // Get the content from the blockstore
        let Ok(content_handle) = fn_sdk::blockstore::ContentHandle::load(&hash).await else {
            eprintln!("failed to load content handle from the blockstore");
            return
        };

        trace!("streaming {} blocks to the client", content_handle.len());

        let bytes = (content_handle.len() as u32).to_be_bytes();
        if let Err(e) = stream.send_payload(bytes.as_slice()).await {
            eprintln!("failed to send number of blocks: {e}");
            return;
        }

        for block in 0..content_handle.len() {
            let Ok(bytes) = content_handle.read(block).await else {
                eprintln!("failed to read content from the blockstore :(");
                return
            };

            if let Err(e) = stream.send_payload(&bytes).await {
                eprintln!("failed to send block: {e}");
                return;
            }
        }
    }
}
