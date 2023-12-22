//! # Fleek Network Fetcher Service
//!
//! ## Request layout:
//!
//! ```text
//! Payload [ origin (u8) . uid (<1024 bytes) ]
//! ```
//!
//! ## Response:
//!
//! Service will send a single u32 counter with the number of blocks for the content.
//! The content will then be streamed in 256KiB payloads.

mod stream;

use arrayref::array_ref;
use fn_sdk::api::Origin as ApiOrigin;
use fn_sdk::connection::Connection;
use stream::ServiceStream;
use tracing::{debug, error, info};

#[derive(Debug)]
#[repr(u8)]
pub enum Origin {
    Blake3 = 0x00,
    IPFS = 0x01,
    Unknown = 0xFF,
}

impl From<u8> for Origin {
    #[inline(always)]
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Blake3,
            1 => Self::IPFS,
            _ => Self::Unknown,
        }
    }
}

impl From<Origin> for ApiOrigin {
    #[inline(always)]
    fn from(val: Origin) -> Self {
        match val {
            Origin::IPFS => ApiOrigin::IPFS,
            _ => unreachable!(),
        }
    }
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    info!("Initialized IPFS fetcher service!");

    let listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(connection_loop(conn));
    }
}

pub async fn connection_loop(conn: Connection) {
    println!("new connection");
    let mut stream = ServiceStream::new(conn);

    while let Some((origin, uri)) = stream.read_request().await {
        println!("got request for cid");

        // Fetch the content from the origin
        let hash = match origin {
            Origin::Unknown => {
                error!("unknown origin");
                return;
            },
            Origin::Blake3 => {
                if uri.len() != 32 {
                    error!("expected a 32 byte hash");
                    return;
                }

                // Fetch the content from the network
                let hash = *array_ref!(uri, 0, 32);
                if !fn_sdk::api::fetch_blake3(hash).await {
                    error!("failed to fetch content");
                    return;
                }

                hash
            },
            origin => {
                // Fetch the content from the origin
                let Some(hash) = fn_sdk::api::fetch_from_origin(origin.into(), uri).await else {
                    error!("failed to fetch from origin");
                    return
                };
                hash
            },
        };

        debug!("downloaded content");

        // Get the content from the blockstore
        let Ok(content_handle) = fn_sdk::blockstore::ContentHandle::load(&hash).await else {
            error!("failed to load content handle from the blockstore");
            return
        };

        debug!("got content handle");

        let bytes = (content_handle.len() as u32).to_be_bytes();
        if let Err(e) = stream.send_payload(bytes.as_slice()).await {
            error!("failed to send number of blocks: {e}");
            return;
        }

        debug!("sent block count {}", content_handle.len());

        for block in 0..content_handle.len() {
            let Ok(bytes) = content_handle.read(block).await else {
                error!("failed to read content from the blockstore :(");
                return
            };

            debug!("sending block {block}");

            if let Err(e) = stream.send_payload(&bytes).await {
                error!("failed to send block: {e}");
                return;
            }
        }
    }
}
