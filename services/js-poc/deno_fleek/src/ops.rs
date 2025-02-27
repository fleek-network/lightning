use std::cell::RefCell;
use std::ffi::c_void;
use std::io::{Read, Seek};
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use arrayref::array_ref;
use b3fs::bucket::POSITION_START_HASHES;
use b3fs::collections::HashTree;
use cid::Cid;
use deno_core::error::{AnyError, JsError};
use deno_core::url::Url;
use deno_core::{op2, v8, ByteString, JsBuffer, OpState, ResourceId};
use deno_error::JsErrorBox;
use deno_http::HttpNextError;
use deno_permissions::ChildPermissionsArg;
use deno_web::{JsMessageData, MessagePortError};
use fleek_crypto::{ClientPublicKey, NodeSignature};
use fn_sdk::blockstore::{block_file, header_file};
use lightning_schema::task_broker::TaskScope;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;
use tracing::debug;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
enum FleekError {
    #[error("Provided public key is invalid")]
    InvalidPublicKey,

    #[error("Provided hash is invalid")]
    InvalidHash,

    #[error("Task Error: {_0}")]
    TaskError(String),

    #[class(inherit)]
    #[error(transparent)]
    FsError(#[from] std::io::Error),

    #[error("{_0:?}")]
    Any(#[from] AnyError),
}

#[op2(async)]
#[serde]
pub async fn run_task(
    state: Rc<RefCell<OpState>>,
    service: u32,
    #[buffer(copy)] body: Vec<u8>,
    #[string] scope: String,
) -> anyhow::Result<Task, FleekError> {
    let scope = TaskScope::from_str(&scope).map_err(FleekError::TaskError)?;

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
    debug!("Runtime: {message}");
}

#[op2(async)]
pub async fn fetch_blake3(#[buffer(copy)] hash: Vec<u8>) -> Result<bool, FleekError> {
    if hash.len() != 32 {
        return Err(FleekError::InvalidHash);
    }

    Ok(fn_sdk::api::fetch_blake3(*array_ref![hash, 0, 32]).await)
}

#[op2(async)]
#[buffer]
pub async fn fetch_from_origin(#[string] raw_url: String) -> Result<Box<[u8]>, FleekError> {
    let url = Url::parse(&raw_url).context("failed to parse origin url")?;
    let (origin, uri) = match url.scheme() {
        // ipfs://bafy...
        "ipfs" => {
            let cid = url.host_str().context("missing host string")?;
            let cid = cid.parse::<Cid>().map_err(|_| FleekError::InvalidHash)?;
            (fn_sdk::api::Origin::IPFS, cid.to_bytes())
        },
        // https://example.com/...#integrity=sha256-...
        "http" | "https" => {
            if !url
                .fragment()
                .context("missing integrity fragment")?
                .starts_with("integrity=")
            {
                return Err(anyhow!("missing integrity hash",).into());
            }

            (fn_sdk::api::Origin::HTTP, raw_url.as_bytes().to_vec())
        },
        _ => return Err(anyhow!("invalid origin scheme",).into()),
    };

    let Some(hash) = fn_sdk::api::fetch_from_origin(origin, uri).await else {
        return Err(anyhow!("failed to fetch {raw_url} from origin").into());
    };

    Ok(hash.to_vec().into_boxed_slice())
}

#[op2(async)]
#[buffer]
pub async fn load_content(#[buffer(copy)] hash: Vec<u8>) -> Result<Box<[u8]>, FleekError> {
    if hash.len() != 32 {
        return Err(FleekError::InvalidHash);
    }

    let path = header_file(array_ref![hash, 0, 32]);

    // TODO: store proof on rust side, and only give javascript an id to reference the handle
    let mut file = std::fs::File::open(path)?;
    file.seek(std::io::SeekFrom::Start(POSITION_START_HASHES as u64))?;
    let mut buffer = vec![];
    let _ = file.read_to_end(&mut buffer)?;

    Ok(buffer.to_vec().into_boxed_slice())
}

#[op2(async)]
#[buffer]
pub async fn read_block(
    #[buffer(copy)] proof: Box<[u8]>,
    #[bigint] index: usize,
) -> Result<Vec<u8>, FleekError> {
    let tree = HashTree::try_from(proof.deref()).context("failed to parse hash tree")?;
    let inner_hash = tree.nth(index);
    let path = block_file(inner_hash);
    let block = std::fs::read(path)?;

    Ok(block)
}

#[op2(async)]
#[string]
pub async fn query_client_flk_balance(
    #[buffer(copy)] address: Vec<u8>,
) -> Result<String, FleekError> {
    if address.len() != 96 {
        return Err(FleekError::InvalidPublicKey);
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
pub async fn query_client_bandwidth_balance(
    #[buffer(copy)] address: Vec<u8>,
) -> Result<String, FleekError> {
    if address.len() != 96 {
        return Err(FleekError::InvalidPublicKey);
    }
    let bytes = *array_ref![address, 0, 96];
    Ok(
        fn_sdk::api::query_client_bandwidth_balance(ClientPublicKey(bytes))
            .await
            .to_string(),
    )
}

#[derive(serde::Serialize)]
pub struct Task {
    responses: Vec<Vec<u8>>,
    signatures: Vec<NodeSignature>,
}

/// Marker type for current task depth
pub struct TaskDepth(pub(crate) u8);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TtyError {
    #[class(inherit)]
    #[error(transparent)]
    Other(#[inherit] JsErrorBox),
}

#[op2(fast)]
pub fn op_set_raw(
    _state: &mut OpState,
    _rid: u32,
    _is_raw: bool,
    _cbreak: bool,
) -> anyhow::Result<(), TtyError> {
    Err(TtyError::Other(JsErrorBox::not_supported()))
}

#[op2(fast)]
pub fn op_can_write_vectored(_state: &mut OpState, #[smi] _rid: ResourceId) -> bool {
    false
}

#[op2(async)]
#[number]
pub async fn op_raw_write_vectored(
    _state: Rc<RefCell<OpState>>,
    #[smi] _rid: ResourceId,
    #[buffer] _buf1: JsBuffer,
    #[buffer] _buf2: JsBuffer,
) -> Result<usize, HttpNextError> {
    Err(HttpNextError::Other(JsErrorBox::not_supported()))
}

#[op2]
#[serde]
pub fn op_bootstrap_unstable_args(_state: &mut OpState) -> Vec<String> {
    unimplemented!()
}

#[op2]
pub fn op_http_set_response_trailers(
    _external: *const c_void,
    #[serde] _trailers: Vec<(ByteString, ByteString)>,
) {
    unimplemented!()
}

#[op2(fast)]
pub fn op_bootstrap_color_depth(_state: &mut OpState) -> i32 {
    unimplemented!()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerArgs {
    _has_source_code: bool,
    _name: Option<String>,
    _permissions: Option<ChildPermissionsArg>,
    _source_code: String,
    _specifier: String,
    _worker_type: WebWorkerType,
    _close_on_idle: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebWorkerType {
    Classic,
    Module,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerId(u32);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CreateWorkerError {
    #[class("DOMExceptionNotSupportedError")]
    #[error("Classic workers are not supported.")]
    ClassicWorkers,
}

#[op2]
#[serde]
pub fn op_create_worker(
    _state: &mut OpState,
    #[serde] _args: CreateWorkerArgs,
    #[serde] _maybe_worker_metadata: Option<JsMessageData>,
) -> Result<WorkerId, CreateWorkerError> {
    Err(CreateWorkerError::ClassicWorkers)
}

#[op2]
pub fn op_host_post_message(
    _state: &mut OpState,
    #[serde] _id: WorkerId,
    #[serde] _data: JsMessageData,
) -> Result<(), MessagePortError> {
    unimplemented!()
}

#[op2(async)]
#[serde]
pub async fn op_host_recv_ctrl(
    _state: Rc<RefCell<OpState>>,
    #[serde] _id: WorkerId,
) -> WorkerControlEvent {
    unimplemented!()
}

#[op2(async)]
#[serde]
pub async fn op_host_recv_message(
    _state: Rc<RefCell<OpState>>,
    #[serde] _id: WorkerId,
) -> Result<Option<JsMessageData>, MessagePortError> {
    unimplemented!()
}

#[op2]
pub fn op_host_terminate_worker(_state: &mut OpState, #[serde] _id: WorkerId) {
    unimplemented!()
}

#[op2(reentrant)]
pub fn op_napi_open<'scope>(
    _scope: &mut v8::HandleScope<'scope>,
    _isolate: *mut v8::Isolate,
    _op_state: Rc<RefCell<OpState>>,
    #[string] _path: String,
    _global: v8::Local<'scope, v8::Object>,
    _buffer_constructor: v8::Local<'scope, v8::Function>,
    _report_error: v8::Local<'scope, v8::Function>,
) -> std::result::Result<v8::Local<'scope, v8::Value>, deno_napi::NApiError> {
    unimplemented!()
}

/// Events that are sent to host from child
/// worker.
#[allow(unused)]
pub enum WorkerControlEvent {
    Error(AnyError),
    TerminalError(AnyError),
    Close,
}

impl Serialize for WorkerControlEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let type_id = match &self {
            WorkerControlEvent::TerminalError(_) => 1_i32,
            WorkerControlEvent::Error(_) => 2_i32,
            WorkerControlEvent::Close => 3_i32,
        };

        match self {
            WorkerControlEvent::TerminalError(error) | WorkerControlEvent::Error(error) => {
                let value = match error.downcast_ref::<JsError>() {
                    Some(js_error) => {
                        let frame = js_error.frames.iter().find(|f| match &f.file_name {
                            Some(s) => !s.trim_start_matches('[').starts_with("ext:"),
                            None => false,
                        });
                        json!({
                          "message": js_error.exception_message,
                          "fileName": frame.map(|f| f.file_name.as_ref()),
                          "lineNumber": frame.map(|f| f.line_number.as_ref()),
                          "columnNumber": frame.map(|f| f.column_number.as_ref()),
                        })
                    },
                    None => json!({
                      "message": error.to_string(),
                    }),
                };

                Serialize::serialize(&(type_id, value), serializer)
            },
            _ => Serialize::serialize(&(type_id, ()), serializer),
        }
    }
}
