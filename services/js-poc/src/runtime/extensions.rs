//! Javascript runtime bindings for the SDK APIs

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use arrayref::array_ref;
use blake3_tree::utils::{tree_index, HashVec};
use cid::Cid;
use deno_core::url::Url;
use deno_core::{extension, op2, OpState};
use fleek_crypto::{ClientPublicKey, NodeSignature};
use fn_sdk::blockstore::get_internal_path;
use lightning_schema::task_broker::TaskScope;
use tracing::info;

use crate::runtime::Permissions;

extension!(
    fleek,
    deps = [
        deno_webidl,
        deno_console,
        deno_url,
        deno_web,
        deno_net,
        deno_fetch,
        deno_websocket,
        deno_crypto,
        deno_webgpu,
        deno_canvas,
        deno_io,
        deno_fs,
        deno_node
    ],
    ops = [
        run_task,
        log,
        fetch_blake3,
        fetch_from_origin,
        load_content,
        read_block,
        query_client_flk_balance,
        query_client_bandwidth_balance
    ],
    options = { depth: u8 },
    state = |state, config| {
        // initialize permissions
        state.put(Permissions {});
        state.put(TaskDepth(config.depth));
    }
);

/// Marker type for current task depth
struct TaskDepth(u8);

#[derive(serde::Serialize)]
pub struct Task {
    responses: Vec<Vec<u8>>,
    signatures: Vec<NodeSignature>,
}

#[op2(async)]
#[serde]
pub async fn run_task(
    state: Rc<RefCell<OpState>>,
    service: u32,
    #[buffer(copy)] body: Vec<u8>,
    #[string] scope: String,
) -> Result<Task> {
    let scope = TaskScope::from_str(&scope)?;

    let depth = {
        let state = state.borrow();
        state.borrow::<TaskDepth>().0
    };

    let (responses, signatures) = fn_sdk::api::run_task(depth + 1, scope, service, body).await;

    Ok(Task {
        responses,
        signatures,
    })
}

#[op2(fast)]
pub fn log(#[string] message: String) {
    info!("Runtime: {message}");
}

#[op2(async)]
pub async fn fetch_blake3(#[buffer(copy)] hash: Vec<u8>) -> Result<bool> {
    if hash.len() != 32 {
        return Err(anyhow!("blake3 hash must be 32 bytes"));
    }

    Ok(fn_sdk::api::fetch_blake3(*array_ref![hash, 0, 32]).await)
}

#[op2(async)]
#[buffer]
pub async fn fetch_from_origin(#[string] raw_url: String) -> Result<Box<[u8]>> {
    let url = Url::parse(&raw_url).context("failed to parse origin url")?;
    let (origin, uri) = match url.scheme() {
        // ipfs://bafy...
        "ipfs" => {
            let cid = url.host_str().context("invalid ipfs hostname")?;
            let cid = cid.parse::<Cid>().context("invalid ipfs cid")?;
            (fn_sdk::api::Origin::IPFS, cid.to_bytes())
        },
        // https://example.com/...#integrity=sha256-...
        "http" | "https" => {
            if !url
                .fragment()
                .context("missing integrity fragment")?
                .starts_with("integrity=")
            {
                bail!("invalid integrity fragment")
            }

            (fn_sdk::api::Origin::HTTP, raw_url.as_bytes().to_vec())
        },
        _ => bail!("invalid origin scheme"),
    };

    let Some(hash) = fn_sdk::api::fetch_from_origin(origin, uri).await else {
        bail!("failed to fetch {raw_url} from origin");
    };

    Ok(hash.to_vec().into_boxed_slice())
}

#[op2(async)]
#[buffer]
pub async fn load_content(#[buffer(copy)] hash: Vec<u8>) -> Result<Box<[u8]>> {
    let path = get_internal_path(array_ref![hash, 0, 32]);

    // TODO: store proof on rust side, and only give javascript an id to reference the handle
    let proof = std::fs::read(path)?.into_boxed_slice();
    if proof.len() & 31 != 0 {
        return Err(anyhow!("corrupted proof in blockstore"));
    }

    Ok(proof)
}

#[op2(async)]
#[buffer]
pub async fn read_block(
    #[buffer(copy)] proof: Box<[u8]>,
    #[bigint] index: usize,
) -> anyhow::Result<Vec<u8>> {
    let tree = HashVec::from_inner(proof);
    let inner_hash = tree[tree_index(index)];
    let path = fn_sdk::blockstore::get_block_path(index, &inner_hash);
    let block = std::fs::read(path)?;

    Ok(block)
}

#[op2(async)]
#[string]
pub async fn query_client_flk_balance(#[buffer(copy)] address: Vec<u8>) -> Result<String> {
    if address.len() != 96 {
        return Err(anyhow!("address must be 32 bytes"));
    }
    let bytes = *array_ref![address, 0, 96];
    Ok(
        fn_sdk::api::query_client_flk_balance(ClientPublicKey(bytes))
            .await
            .to_string(),
    )
}

#[op2(async)]
#[string]
pub async fn query_client_bandwidth_balance(#[buffer(copy)] address: Vec<u8>) -> Result<String> {
    if address.len() != 96 {
        return Err(anyhow!("address must be 32 bytes"));
    }
    let bytes = *array_ref![address, 0, 96];
    Ok(
        fn_sdk::api::query_client_bandwidth_balance(ClientPublicKey(bytes))
            .await
            .to_string(),
    )
}
