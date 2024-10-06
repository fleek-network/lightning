use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use deno_core::error::{AnyError, JsError};
use deno_core::{op2, ByteString, JsBuffer, OpState, ResourceId, v8};
use deno_napi::NapiPermissions;
use deno_permissions::ChildPermissionsArg;
use deno_web::JsMessageData;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;

// Here
#[op2(fast)]
pub fn op_set_raw(
    state: &mut OpState,
    rid: u32,
    is_raw: bool,
    cbreak: bool,
) -> anyhow::Result<(), AnyError> {
    todo!()
}

#[op2(fast)]
pub fn op_can_write_vectored(state: &mut OpState, #[smi] rid: ResourceId) -> bool {
    todo!()
}

#[op2(async)]
#[number]
pub async fn op_raw_write_vectored(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
    #[buffer] buf1: JsBuffer,
    #[buffer] buf2: JsBuffer,
) -> anyhow::Result<usize, AnyError> {
    todo!()
}

#[op2]
#[serde]
pub fn op_bootstrap_unstable_args(state: &mut OpState) -> Vec<String> {
    todo!()
}

#[op2]
pub fn op_http_set_response_trailers(
    external: *const c_void,
    #[serde] trailers: Vec<(ByteString, ByteString)>,
) {
    todo!()
}

#[op2(fast)]
pub fn op_bootstrap_color_depth(state: &mut OpState) -> i32 {
    todo!()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerArgs {
    has_source_code: bool,
    name: Option<String>,
    permissions: Option<ChildPermissionsArg>,
    source_code: String,
    specifier: String,
    worker_type: WebWorkerType,
    close_on_idle: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebWorkerType {
    Classic,
    Module,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerId(u32);

#[op2]
#[serde]
pub fn op_create_worker(
    state: &mut OpState,
    #[serde] args: CreateWorkerArgs,
    #[serde] maybe_worker_metadata: Option<JsMessageData>,
) -> anyhow::Result<WorkerId, AnyError> {
    todo!()
}

#[op2]
pub fn op_host_post_message(
    state: &mut OpState,
    #[serde] id: WorkerId,
    #[serde] data: JsMessageData,
) -> anyhow::Result<(), AnyError> {
    todo!()
}

#[op2(async)]
#[serde]
pub async fn op_host_recv_ctrl(
    state: Rc<RefCell<OpState>>,
    #[serde] id: WorkerId,
) -> anyhow::Result<WorkerControlEvent, AnyError> {
    todo!()
}

#[op2(async)]
#[serde]
pub async fn op_host_recv_message(
    state: Rc<RefCell<OpState>>,
    #[serde] id: WorkerId,
) -> anyhow::Result<Option<JsMessageData>, AnyError> {
    todo!()
}

#[op2]
pub fn op_host_terminate_worker(state: &mut OpState, #[serde] id: WorkerId) {
    todo!()
}

#[op2(reentrant)]
pub fn op_napi_open<NP, 'scope>(
    scope: &mut v8::HandleScope<'scope>,
    isolate: *mut v8::Isolate,
    op_state: Rc<RefCell<OpState>>,
    #[string] path: String,
    global: v8::Local<'scope, v8::Object>,
    buffer_constructor: v8::Local<'scope, v8::Function>,
    report_error: v8::Local<'scope, v8::Function>,
) -> std::result::Result<v8::Local<'scope, v8::Value>, AnyError>
where
    NP: NapiPermissions + 'static,
{
    todo!()
}

/// Events that are sent to host from child
/// worker.
pub enum WorkerControlEvent {
    Error(AnyError),
    TerminalError(AnyError),
    Close,
}

impl Serialize for WorkerControlEvent {
    fn serialize<S>(&self, serializer: S) -> anyhow::Result<S::Ok, S::Error>
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
