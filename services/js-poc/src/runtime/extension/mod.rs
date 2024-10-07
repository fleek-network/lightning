pub mod ops;
pub mod permissions;
pub mod params {
    use std::time::Duration;

    pub const HEAP_INIT: usize = 1 << 10;
    pub const HEAP_LIMIT: usize = 50 << 20;
    pub const REQ_TIMEOUT: Duration = Duration::from_secs(15);
    pub const FETCH_BLACKLIST: &[&str] = &["localhost", "127.0.0.1", "::1"];
}

use deno_core::extension;
use deno_napi::NapiPermissions;
use ops::{
    fetch_blake3,
    fetch_from_origin,
    load_content,
    log,
    op_bootstrap_color_depth,
    op_bootstrap_unstable_args,
    op_can_write_vectored,
    op_create_worker,
    op_host_post_message,
    op_host_recv_ctrl,
    op_host_recv_message,
    op_host_terminate_worker,
    op_http_set_response_trailers,
    op_napi_open,
    op_raw_write_vectored,
    op_set_raw,
    query_client_bandwidth_balance,
    query_client_flk_balance,
    read_block,
    run_task,
    TaskDepth,
};
use permissions::Permissions;

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
    parameters = [P: NapiPermissions],
    ops = [
        run_task,
        log,
        fetch_blake3,
        fetch_from_origin,
        load_content,
        read_block,
        query_client_flk_balance,
        query_client_bandwidth_balance,
        op_set_raw,
        op_can_write_vectored,
        op_raw_write_vectored,
        op_bootstrap_unstable_args,
        op_http_set_response_trailers,
        op_bootstrap_color_depth,
        op_create_worker,
        op_host_post_message,
        op_host_recv_ctrl,
        op_host_recv_message,
        op_host_terminate_worker,
        op_napi_open<P>
    ],
    esm_entry_point = "ext:fleek/bootstrap.js",
    esm = [
        dir "src/runtime/js",
        "fleek.js",
        "global.js",
        "bootstrap.js",
        "ext:runtime/98_global_scope_shared.js" = "98_global_scope_shared.js",
        "ext:deno_http/00_serve.ts" = "00_serve.ts",
        "ext:runtime/30_os.js" = "30_os.js",
        "ext:deno_broadcast_channel/01_broadcast_channel.js" = "01_broadcast_channel.js",
        "ext:deno_fs/30_fs.js" = "30_fs.js"
    ],
    options = { depth: u8 },
    state = |state, config| {
        // initialize permissions
        state.put(Permissions {});
        state.put(TaskDepth(config.depth));
    }
);
