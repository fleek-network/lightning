use deno_core::extension;

use crate::ops::{
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
use crate::permissions::Permissions;

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
        op_napi_open
    ],
    esm_entry_point = "ext:fleek/bootstrap.js",
    esm = [
        dir "src/js",
        "fleek.js",
        "global.js",
        "bootstrap.js",
        "ext:runtime/98_global_scope_shared.js" = "98_global_scope_shared.js",
        "ext:deno_http/00_serve.ts" = "00_serve.ts",
        "ext:deno_os/30_os.js" = "30_os.js",
        "ext:deno_broadcast_channel/01_broadcast_channel.js" = "01_broadcast_channel.js",
        "ext:deno_fs/30_fs.js" = "30_fs.js",
        "ext:deno_process/40_process.js" = "40_process.js",
    ],
    options = { depth: u8 },
    state = |state, config| {
        // initialize permissions
        state.put(Permissions {});
        state.put(TaskDepth(config.depth));
    }
);
